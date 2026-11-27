//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

/// The header's `D3D12_AUTO_BREADCRUMB_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AutoBreadcrumbOp(pub i32);

/// The header's `D3D12_AUTO_BREADCRUMB_OP_SETMARKER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_SETMARKER: AutoBreadcrumbOp = AutoBreadcrumbOp(0);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_BEGINEVENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_BEGINEVENT: AutoBreadcrumbOp = AutoBreadcrumbOp(1);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_ENDEVENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_ENDEVENT: AutoBreadcrumbOp = AutoBreadcrumbOp(2);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DRAWINSTANCED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DRAWINSTANCED: AutoBreadcrumbOp = AutoBreadcrumbOp(3);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DRAWINDEXEDINSTANCED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DRAWINDEXEDINSTANCED: AutoBreadcrumbOp = AutoBreadcrumbOp(4);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_EXECUTEINDIRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_EXECUTEINDIRECT: AutoBreadcrumbOp = AutoBreadcrumbOp(5);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DISPATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DISPATCH: AutoBreadcrumbOp = AutoBreadcrumbOp(6);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_COPYBUFFERREGION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_COPYBUFFERREGION: AutoBreadcrumbOp = AutoBreadcrumbOp(7);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_COPYTEXTUREREGION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_COPYTEXTUREREGION: AutoBreadcrumbOp = AutoBreadcrumbOp(8);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_COPYRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_COPYRESOURCE: AutoBreadcrumbOp = AutoBreadcrumbOp(9);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_COPYTILES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_COPYTILES: AutoBreadcrumbOp = AutoBreadcrumbOp(10);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_RESOLVESUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_RESOLVESUBRESOURCE: AutoBreadcrumbOp = AutoBreadcrumbOp(11);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_CLEARRENDERTARGETVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_CLEARRENDERTARGETVIEW: AutoBreadcrumbOp = AutoBreadcrumbOp(12);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_CLEARUNORDEREDACCESSVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_CLEARUNORDEREDACCESSVIEW: AutoBreadcrumbOp =
    AutoBreadcrumbOp(13);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_CLEARDEPTHSTENCILVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_CLEARDEPTHSTENCILVIEW: AutoBreadcrumbOp = AutoBreadcrumbOp(14);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_RESOURCEBARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_RESOURCEBARRIER: AutoBreadcrumbOp = AutoBreadcrumbOp(15);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_EXECUTEBUNDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_EXECUTEBUNDLE: AutoBreadcrumbOp = AutoBreadcrumbOp(16);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_PRESENT: AutoBreadcrumbOp = AutoBreadcrumbOp(17);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_RESOLVEQUERYDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_RESOLVEQUERYDATA: AutoBreadcrumbOp = AutoBreadcrumbOp(18);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_BEGINSUBMISSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_BEGINSUBMISSION: AutoBreadcrumbOp = AutoBreadcrumbOp(19);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_ENDSUBMISSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_ENDSUBMISSION: AutoBreadcrumbOp = AutoBreadcrumbOp(20);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DECODEFRAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DECODEFRAME: AutoBreadcrumbOp = AutoBreadcrumbOp(21);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_PROCESSFRAMES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_PROCESSFRAMES: AutoBreadcrumbOp = AutoBreadcrumbOp(22);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_ATOMICCOPYBUFFERUINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_ATOMICCOPYBUFFERUINT: AutoBreadcrumbOp = AutoBreadcrumbOp(23);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_ATOMICCOPYBUFFERUINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_ATOMICCOPYBUFFERUINT64: AutoBreadcrumbOp = AutoBreadcrumbOp(24);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_RESOLVESUBRESOURCEREGION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_RESOLVESUBRESOURCEREGION: AutoBreadcrumbOp =
    AutoBreadcrumbOp(25);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_WRITEBUFFERIMMEDIATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_WRITEBUFFERIMMEDIATE: AutoBreadcrumbOp = AutoBreadcrumbOp(26);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DECODEFRAME1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DECODEFRAME1: AutoBreadcrumbOp = AutoBreadcrumbOp(27);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_SETPROTECTEDRESOURCESESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_SETPROTECTEDRESOURCESESSION: AutoBreadcrumbOp =
    AutoBreadcrumbOp(28);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DECODEFRAME2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DECODEFRAME2: AutoBreadcrumbOp = AutoBreadcrumbOp(29);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_PROCESSFRAMES1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_PROCESSFRAMES1: AutoBreadcrumbOp = AutoBreadcrumbOp(30);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_BUILDRAYTRACINGACCELERATIONSTRUCTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_BUILDRAYTRACINGACCELERATIONSTRUCTURE: AutoBreadcrumbOp =
    AutoBreadcrumbOp(31);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_EMITRAYTRACINGACCELERATIONSTRUCTUREPOSTBUILDINFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_EMITRAYTRACINGACCELERATIONSTRUCTUREPOSTBUILDINFO:
    AutoBreadcrumbOp = AutoBreadcrumbOp(32);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_COPYRAYTRACINGACCELERATIONSTRUCTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_COPYRAYTRACINGACCELERATIONSTRUCTURE: AutoBreadcrumbOp =
    AutoBreadcrumbOp(33);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DISPATCHRAYS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DISPATCHRAYS: AutoBreadcrumbOp = AutoBreadcrumbOp(34);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_INITIALIZEMETACOMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_INITIALIZEMETACOMMAND: AutoBreadcrumbOp = AutoBreadcrumbOp(35);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_EXECUTEMETACOMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_EXECUTEMETACOMMAND: AutoBreadcrumbOp = AutoBreadcrumbOp(36);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_ESTIMATEMOTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_ESTIMATEMOTION: AutoBreadcrumbOp = AutoBreadcrumbOp(37);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_RESOLVEMOTIONVECTORHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_RESOLVEMOTIONVECTORHEAP: AutoBreadcrumbOp = AutoBreadcrumbOp(38);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_SETPIPELINESTATE1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_SETPIPELINESTATE1: AutoBreadcrumbOp = AutoBreadcrumbOp(39);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_INITIALIZEEXTENSIONCOMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_INITIALIZEEXTENSIONCOMMAND: AutoBreadcrumbOp =
    AutoBreadcrumbOp(40);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_EXECUTEEXTENSIONCOMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_EXECUTEEXTENSIONCOMMAND: AutoBreadcrumbOp = AutoBreadcrumbOp(41);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DISPATCHMESH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DISPATCHMESH: AutoBreadcrumbOp = AutoBreadcrumbOp(42);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_ENCODEFRAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_ENCODEFRAME: AutoBreadcrumbOp = AutoBreadcrumbOp(43);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_RESOLVEENCODEROUTPUTMETADATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_RESOLVEENCODEROUTPUTMETADATA: AutoBreadcrumbOp =
    AutoBreadcrumbOp(44);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_BARRIER: AutoBreadcrumbOp = AutoBreadcrumbOp(45);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_BEGIN_COMMAND_LIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_BEGIN_COMMAND_LIST: AutoBreadcrumbOp = AutoBreadcrumbOp(46);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_DISPATCHGRAPH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_DISPATCHGRAPH: AutoBreadcrumbOp = AutoBreadcrumbOp(47);
/// The header's `D3D12_AUTO_BREADCRUMB_OP_SETPROGRAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_auto_breadcrumb_op).
pub const D3D12_AUTO_BREADCRUMB_OP_SETPROGRAM: AutoBreadcrumbOp = AutoBreadcrumbOp(48);

/// The header's `D3D12_AXIS_SHADING_RATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_axis_shading_rate).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AxisShadingRate(pub i32);

/// The header's `D3D12_AXIS_SHADING_RATE_1X`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_axis_shading_rate).
pub const D3D12_AXIS_SHADING_RATE_1X: AxisShadingRate = AxisShadingRate(0);
/// The header's `D3D12_AXIS_SHADING_RATE_2X`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_axis_shading_rate).
pub const D3D12_AXIS_SHADING_RATE_2X: AxisShadingRate = AxisShadingRate(1);
/// The header's `D3D12_AXIS_SHADING_RATE_4X`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_axis_shading_rate).
pub const D3D12_AXIS_SHADING_RATE_4X: AxisShadingRate = AxisShadingRate(2);

/// The header's `D3D12_BACKGROUND_PROCESSING_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_background_processing_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BackgroundProcessingMode(pub i32);

/// The header's `D3D12_BACKGROUND_PROCESSING_MODE_ALLOWED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_background_processing_mode).
pub const D3D12_BACKGROUND_PROCESSING_MODE_ALLOWED: BackgroundProcessingMode =
    BackgroundProcessingMode(0);
/// The header's `D3D12_BACKGROUND_PROCESSING_MODE_ALLOW_INTRUSIVE_MEASUREMENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_background_processing_mode).
pub const D3D12_BACKGROUND_PROCESSING_MODE_ALLOW_INTRUSIVE_MEASUREMENTS: BackgroundProcessingMode =
    BackgroundProcessingMode(1);
/// The header's `D3D12_BACKGROUND_PROCESSING_MODE_DISABLE_BACKGROUND_WORK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_background_processing_mode).
pub const D3D12_BACKGROUND_PROCESSING_MODE_DISABLE_BACKGROUND_WORK: BackgroundProcessingMode =
    BackgroundProcessingMode(2);
/// The header's `D3D12_BACKGROUND_PROCESSING_MODE_DISABLE_PROFILING_BY_SYSTEM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_background_processing_mode).
pub const D3D12_BACKGROUND_PROCESSING_MODE_DISABLE_PROFILING_BY_SYSTEM: BackgroundProcessingMode =
    BackgroundProcessingMode(3);

/// The header's `D3D12_BARRIER_ACCESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BarrierAccess(pub i32);

/// The header's `D3D12_BARRIER_ACCESS_COMMON`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_COMMON: BarrierAccess = BarrierAccess(0);
/// The header's `D3D12_BARRIER_ACCESS_VERTEX_BUFFER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_VERTEX_BUFFER: BarrierAccess = BarrierAccess(1);
/// The header's `D3D12_BARRIER_ACCESS_CONSTANT_BUFFER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_CONSTANT_BUFFER: BarrierAccess = BarrierAccess(2);
/// The header's `D3D12_BARRIER_ACCESS_INDEX_BUFFER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_INDEX_BUFFER: BarrierAccess = BarrierAccess(4);
/// The header's `D3D12_BARRIER_ACCESS_RENDER_TARGET`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_RENDER_TARGET: BarrierAccess = BarrierAccess(8);
/// The header's `D3D12_BARRIER_ACCESS_UNORDERED_ACCESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_UNORDERED_ACCESS: BarrierAccess = BarrierAccess(16);
/// The header's `D3D12_BARRIER_ACCESS_DEPTH_STENCIL_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_DEPTH_STENCIL_WRITE: BarrierAccess = BarrierAccess(32);
/// The header's `D3D12_BARRIER_ACCESS_DEPTH_STENCIL_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_DEPTH_STENCIL_READ: BarrierAccess = BarrierAccess(64);
/// The header's `D3D12_BARRIER_ACCESS_SHADER_RESOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_SHADER_RESOURCE: BarrierAccess = BarrierAccess(128);
/// The header's `D3D12_BARRIER_ACCESS_STREAM_OUTPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_STREAM_OUTPUT: BarrierAccess = BarrierAccess(0x100);
/// The header's `D3D12_BARRIER_ACCESS_INDIRECT_ARGUMENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_INDIRECT_ARGUMENT: BarrierAccess = BarrierAccess(0x200);
/// The header's `D3D12_BARRIER_ACCESS_PREDICATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_PREDICATION: BarrierAccess = BarrierAccess(0x200);
/// The header's `D3D12_BARRIER_ACCESS_COPY_DEST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_COPY_DEST: BarrierAccess = BarrierAccess(0x400);
/// The header's `D3D12_BARRIER_ACCESS_COPY_SOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_COPY_SOURCE: BarrierAccess = BarrierAccess(0x800);
/// The header's `D3D12_BARRIER_ACCESS_RESOLVE_DEST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_RESOLVE_DEST: BarrierAccess = BarrierAccess(0x1000);
/// The header's `D3D12_BARRIER_ACCESS_RESOLVE_SOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_RESOLVE_SOURCE: BarrierAccess = BarrierAccess(0x2000);
/// The header's `D3D12_BARRIER_ACCESS_RAYTRACING_ACCELERATION_STRUCTURE_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_RAYTRACING_ACCELERATION_STRUCTURE_READ: BarrierAccess =
    BarrierAccess(0x4000);
/// The header's `D3D12_BARRIER_ACCESS_RAYTRACING_ACCELERATION_STRUCTURE_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_RAYTRACING_ACCELERATION_STRUCTURE_WRITE: BarrierAccess =
    BarrierAccess(0x8000);
/// The header's `D3D12_BARRIER_ACCESS_SHADING_RATE_SOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_SHADING_RATE_SOURCE: BarrierAccess = BarrierAccess(0x10000);
/// The header's `D3D12_BARRIER_ACCESS_VIDEO_DECODE_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_VIDEO_DECODE_READ: BarrierAccess = BarrierAccess(0x20000);
/// The header's `D3D12_BARRIER_ACCESS_VIDEO_DECODE_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_VIDEO_DECODE_WRITE: BarrierAccess = BarrierAccess(0x40000);
/// The header's `D3D12_BARRIER_ACCESS_VIDEO_PROCESS_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_VIDEO_PROCESS_READ: BarrierAccess = BarrierAccess(0x80000);
/// The header's `D3D12_BARRIER_ACCESS_VIDEO_PROCESS_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_VIDEO_PROCESS_WRITE: BarrierAccess = BarrierAccess(0x100000);
/// The header's `D3D12_BARRIER_ACCESS_VIDEO_ENCODE_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_VIDEO_ENCODE_READ: BarrierAccess = BarrierAccess(0x200000);
/// The header's `D3D12_BARRIER_ACCESS_VIDEO_ENCODE_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_VIDEO_ENCODE_WRITE: BarrierAccess = BarrierAccess(0x400000);
/// The header's `D3D12_BARRIER_ACCESS_NO_ACCESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_ACCESS_NO_ACCESS: BarrierAccess = BarrierAccess(-2147483648);

/// The header's `D3D12_BARRIER_LAYOUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BarrierLayout(pub i32);

/// The header's `D3D12_BARRIER_LAYOUT_UNDEFINED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_UNDEFINED: BarrierLayout = BarrierLayout(-1);
/// The header's `D3D12_BARRIER_LAYOUT_COMMON`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COMMON: BarrierLayout = BarrierLayout(0);
/// The header's `D3D12_BARRIER_LAYOUT_PRESENT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_PRESENT: BarrierLayout = BarrierLayout(0);
/// The header's `D3D12_BARRIER_LAYOUT_GENERIC_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_GENERIC_READ: BarrierLayout = BarrierLayout(1);
/// The header's `D3D12_BARRIER_LAYOUT_RENDER_TARGET`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_RENDER_TARGET: BarrierLayout = BarrierLayout(2);
/// The header's `D3D12_BARRIER_LAYOUT_UNORDERED_ACCESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_UNORDERED_ACCESS: BarrierLayout = BarrierLayout(3);
/// The header's `D3D12_BARRIER_LAYOUT_DEPTH_STENCIL_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_DEPTH_STENCIL_WRITE: BarrierLayout = BarrierLayout(4);
/// The header's `D3D12_BARRIER_LAYOUT_DEPTH_STENCIL_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_DEPTH_STENCIL_READ: BarrierLayout = BarrierLayout(5);
/// The header's `D3D12_BARRIER_LAYOUT_SHADER_RESOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_SHADER_RESOURCE: BarrierLayout = BarrierLayout(6);
/// The header's `D3D12_BARRIER_LAYOUT_COPY_SOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COPY_SOURCE: BarrierLayout = BarrierLayout(7);
/// The header's `D3D12_BARRIER_LAYOUT_COPY_DEST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COPY_DEST: BarrierLayout = BarrierLayout(8);
/// The header's `D3D12_BARRIER_LAYOUT_RESOLVE_SOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_RESOLVE_SOURCE: BarrierLayout = BarrierLayout(9);
/// The header's `D3D12_BARRIER_LAYOUT_RESOLVE_DEST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_RESOLVE_DEST: BarrierLayout = BarrierLayout(10);
/// The header's `D3D12_BARRIER_LAYOUT_SHADING_RATE_SOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_SHADING_RATE_SOURCE: BarrierLayout = BarrierLayout(11);
/// The header's `D3D12_BARRIER_LAYOUT_VIDEO_DECODE_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_VIDEO_DECODE_READ: BarrierLayout = BarrierLayout(12);
/// The header's `D3D12_BARRIER_LAYOUT_VIDEO_DECODE_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_VIDEO_DECODE_WRITE: BarrierLayout = BarrierLayout(13);
/// The header's `D3D12_BARRIER_LAYOUT_VIDEO_PROCESS_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_VIDEO_PROCESS_READ: BarrierLayout = BarrierLayout(14);
/// The header's `D3D12_BARRIER_LAYOUT_VIDEO_PROCESS_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_VIDEO_PROCESS_WRITE: BarrierLayout = BarrierLayout(15);
/// The header's `D3D12_BARRIER_LAYOUT_VIDEO_ENCODE_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_VIDEO_ENCODE_READ: BarrierLayout = BarrierLayout(16);
/// The header's `D3D12_BARRIER_LAYOUT_VIDEO_ENCODE_WRITE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_VIDEO_ENCODE_WRITE: BarrierLayout = BarrierLayout(17);
/// The header's `D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_COMMON`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_COMMON: BarrierLayout = BarrierLayout(18);
/// The header's `D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_GENERIC_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_GENERIC_READ: BarrierLayout = BarrierLayout(19);
/// The header's `D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_UNORDERED_ACCESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_UNORDERED_ACCESS: BarrierLayout = BarrierLayout(20);
/// The header's `D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_SHADER_RESOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_SHADER_RESOURCE: BarrierLayout = BarrierLayout(21);
/// The header's `D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_COPY_SOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_COPY_SOURCE: BarrierLayout = BarrierLayout(22);
/// The header's `D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_COPY_DEST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_DIRECT_QUEUE_COPY_DEST: BarrierLayout = BarrierLayout(23);
/// The header's `D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_COMMON`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_COMMON: BarrierLayout = BarrierLayout(24);
/// The header's `D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_GENERIC_READ`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_GENERIC_READ: BarrierLayout = BarrierLayout(25);
/// The header's `D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_UNORDERED_ACCESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_UNORDERED_ACCESS: BarrierLayout = BarrierLayout(26);
/// The header's `D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_SHADER_RESOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_SHADER_RESOURCE: BarrierLayout = BarrierLayout(27);
/// The header's `D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_COPY_SOURCE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_COPY_SOURCE: BarrierLayout = BarrierLayout(28);
/// The header's `D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_COPY_DEST`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_COMPUTE_QUEUE_COPY_DEST: BarrierLayout = BarrierLayout(29);
/// The header's `D3D12_BARRIER_LAYOUT_VIDEO_QUEUE_COMMON`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_LAYOUT_VIDEO_QUEUE_COMMON: BarrierLayout = BarrierLayout(30);

/// The header's `D3D12_BARRIER_SYNC`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BarrierSync(pub i32);

/// The header's `D3D12_BARRIER_SYNC_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_NONE: BarrierSync = BarrierSync(0);
/// The header's `D3D12_BARRIER_SYNC_ALL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_ALL: BarrierSync = BarrierSync(1);
/// The header's `D3D12_BARRIER_SYNC_DRAW`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_DRAW: BarrierSync = BarrierSync(2);
/// The header's `D3D12_BARRIER_SYNC_INDEX_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_INDEX_INPUT: BarrierSync = BarrierSync(4);
/// The header's `D3D12_BARRIER_SYNC_VERTEX_SHADING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_VERTEX_SHADING: BarrierSync = BarrierSync(8);
/// The header's `D3D12_BARRIER_SYNC_PIXEL_SHADING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_PIXEL_SHADING: BarrierSync = BarrierSync(16);
/// The header's `D3D12_BARRIER_SYNC_DEPTH_STENCIL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_DEPTH_STENCIL: BarrierSync = BarrierSync(32);
/// The header's `D3D12_BARRIER_SYNC_RENDER_TARGET`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_RENDER_TARGET: BarrierSync = BarrierSync(64);
/// The header's `D3D12_BARRIER_SYNC_COMPUTE_SHADING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_COMPUTE_SHADING: BarrierSync = BarrierSync(128);
/// The header's `D3D12_BARRIER_SYNC_RAYTRACING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_RAYTRACING: BarrierSync = BarrierSync(0x100);
/// The header's `D3D12_BARRIER_SYNC_COPY`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_COPY: BarrierSync = BarrierSync(0x200);
/// The header's `D3D12_BARRIER_SYNC_RESOLVE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_RESOLVE: BarrierSync = BarrierSync(0x400);
/// The header's `D3D12_BARRIER_SYNC_EXECUTE_INDIRECT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_EXECUTE_INDIRECT: BarrierSync = BarrierSync(0x800);
/// The header's `D3D12_BARRIER_SYNC_PREDICATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_PREDICATION: BarrierSync = BarrierSync(0x800);
/// The header's `D3D12_BARRIER_SYNC_ALL_SHADING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_ALL_SHADING: BarrierSync = BarrierSync(0x1000);
/// The header's `D3D12_BARRIER_SYNC_NON_PIXEL_SHADING`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_NON_PIXEL_SHADING: BarrierSync = BarrierSync(0x2000);
/// The header's `D3D12_BARRIER_SYNC_EMIT_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_EMIT_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO: BarrierSync =
    BarrierSync(0x4000);
/// The header's `D3D12_BARRIER_SYNC_CLEAR_UNORDERED_ACCESS_VIEW`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_CLEAR_UNORDERED_ACCESS_VIEW: BarrierSync = BarrierSync(0x8000);
/// The header's `D3D12_BARRIER_SYNC_VIDEO_DECODE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_VIDEO_DECODE: BarrierSync = BarrierSync(0x100000);
/// The header's `D3D12_BARRIER_SYNC_VIDEO_PROCESS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_VIDEO_PROCESS: BarrierSync = BarrierSync(0x200000);
/// The header's `D3D12_BARRIER_SYNC_VIDEO_ENCODE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_VIDEO_ENCODE: BarrierSync = BarrierSync(0x400000);
/// The header's `D3D12_BARRIER_SYNC_BUILD_RAYTRACING_ACCELERATION_STRUCTURE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_BUILD_RAYTRACING_ACCELERATION_STRUCTURE: BarrierSync =
    BarrierSync(0x800000);
/// The header's `D3D12_BARRIER_SYNC_COPY_RAYTRACING_ACCELERATION_STRUCTURE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_COPY_RAYTRACING_ACCELERATION_STRUCTURE: BarrierSync =
    BarrierSync(0x1000000);
/// The header's `D3D12_BARRIER_SYNC_SPLIT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_SYNC_SPLIT: BarrierSync = BarrierSync(-2147483648);

/// The header's `D3D12_BARRIER_TYPE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BarrierType(pub i32);

/// The header's `D3D12_BARRIER_TYPE_GLOBAL`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_TYPE_GLOBAL: BarrierType = BarrierType(0);
/// The header's `D3D12_BARRIER_TYPE_TEXTURE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_TYPE_TEXTURE: BarrierType = BarrierType(1);
/// The header's `D3D12_BARRIER_TYPE_BUFFER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_BARRIER_TYPE_BUFFER: BarrierType = BarrierType(2);

/// The header's `D3D12_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Blend(pub i32);

/// The header's `D3D12_BLEND_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_ZERO: Blend = Blend(1);
/// The header's `D3D12_BLEND_ONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_ONE: Blend = Blend(2);
/// The header's `D3D12_BLEND_SRC_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_SRC_COLOR: Blend = Blend(3);
/// The header's `D3D12_BLEND_INV_SRC_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_INV_SRC_COLOR: Blend = Blend(4);
/// The header's `D3D12_BLEND_SRC_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_SRC_ALPHA: Blend = Blend(5);
/// The header's `D3D12_BLEND_INV_SRC_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_INV_SRC_ALPHA: Blend = Blend(6);
/// The header's `D3D12_BLEND_DEST_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_DEST_ALPHA: Blend = Blend(7);
/// The header's `D3D12_BLEND_INV_DEST_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_INV_DEST_ALPHA: Blend = Blend(8);
/// The header's `D3D12_BLEND_DEST_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_DEST_COLOR: Blend = Blend(9);
/// The header's `D3D12_BLEND_INV_DEST_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_INV_DEST_COLOR: Blend = Blend(10);
/// The header's `D3D12_BLEND_SRC_ALPHA_SAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_SRC_ALPHA_SAT: Blend = Blend(11);
/// The header's `D3D12_BLEND_BLEND_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_BLEND_FACTOR: Blend = Blend(14);
/// The header's `D3D12_BLEND_INV_BLEND_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_INV_BLEND_FACTOR: Blend = Blend(15);
/// The header's `D3D12_BLEND_SRC1_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_SRC1_COLOR: Blend = Blend(16);
/// The header's `D3D12_BLEND_INV_SRC1_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_INV_SRC1_COLOR: Blend = Blend(17);
/// The header's `D3D12_BLEND_SRC1_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_SRC1_ALPHA: Blend = Blend(18);
/// The header's `D3D12_BLEND_INV_SRC1_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_INV_SRC1_ALPHA: Blend = Blend(19);
/// The header's `D3D12_BLEND_ALPHA_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_ALPHA_FACTOR: Blend = Blend(20);
/// The header's `D3D12_BLEND_INV_ALPHA_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend).
pub const D3D12_BLEND_INV_ALPHA_FACTOR: Blend = Blend(21);

/// The header's `D3D12_BLEND_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend_op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BlendOp(pub i32);

/// The header's `D3D12_BLEND_OP_ADD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend_op).
pub const D3D12_BLEND_OP_ADD: BlendOp = BlendOp(1);
/// The header's `D3D12_BLEND_OP_SUBTRACT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend_op).
pub const D3D12_BLEND_OP_SUBTRACT: BlendOp = BlendOp(2);
/// The header's `D3D12_BLEND_OP_REV_SUBTRACT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend_op).
pub const D3D12_BLEND_OP_REV_SUBTRACT: BlendOp = BlendOp(3);
/// The header's `D3D12_BLEND_OP_MIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend_op).
pub const D3D12_BLEND_OP_MIN: BlendOp = BlendOp(4);
/// The header's `D3D12_BLEND_OP_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_blend_op).
pub const D3D12_BLEND_OP_MAX: BlendOp = BlendOp(5);

/// The header's `D3D12_BUFFER_SRV_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_buffer_srv_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BufferSrvFlags(pub i32);

/// The header's `D3D12_BUFFER_SRV_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_buffer_srv_flags).
pub const D3D12_BUFFER_SRV_FLAG_NONE: BufferSrvFlags = BufferSrvFlags(0);
/// The header's `D3D12_BUFFER_SRV_FLAG_RAW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_buffer_srv_flags).
pub const D3D12_BUFFER_SRV_FLAG_RAW: BufferSrvFlags = BufferSrvFlags(1);

/// The header's `D3D12_BUFFER_UAV_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_buffer_uav_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BufferUavFlags(pub i32);

/// The header's `D3D12_BUFFER_UAV_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_buffer_uav_flags).
pub const D3D12_BUFFER_UAV_FLAG_NONE: BufferUavFlags = BufferUavFlags(0);
/// The header's `D3D12_BUFFER_UAV_FLAG_RAW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_buffer_uav_flags).
pub const D3D12_BUFFER_UAV_FLAG_RAW: BufferUavFlags = BufferUavFlags(1);

/// The header's `D3D12_CLEAR_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_clear_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClearFlags(pub i32);

/// The header's `D3D12_CLEAR_FLAG_DEPTH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_clear_flags).
pub const D3D12_CLEAR_FLAG_DEPTH: ClearFlags = ClearFlags(1);
/// The header's `D3D12_CLEAR_FLAG_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_clear_flags).
pub const D3D12_CLEAR_FLAG_STENCIL: ClearFlags = ClearFlags(2);

/// The header's `D3D12_COLOR_WRITE_ENABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_color_write_enable).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ColorWriteEnable(pub i32);

/// The header's `D3D12_COLOR_WRITE_ENABLE_RED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_color_write_enable).
pub const D3D12_COLOR_WRITE_ENABLE_RED: ColorWriteEnable = ColorWriteEnable(1);
/// The header's `D3D12_COLOR_WRITE_ENABLE_GREEN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_color_write_enable).
pub const D3D12_COLOR_WRITE_ENABLE_GREEN: ColorWriteEnable = ColorWriteEnable(2);
/// The header's `D3D12_COLOR_WRITE_ENABLE_BLUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_color_write_enable).
pub const D3D12_COLOR_WRITE_ENABLE_BLUE: ColorWriteEnable = ColorWriteEnable(4);
/// The header's `D3D12_COLOR_WRITE_ENABLE_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_color_write_enable).
pub const D3D12_COLOR_WRITE_ENABLE_ALPHA: ColorWriteEnable = ColorWriteEnable(8);
/// The header's `D3D12_COLOR_WRITE_ENABLE_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_color_write_enable).
pub const D3D12_COLOR_WRITE_ENABLE_ALL: ColorWriteEnable = ColorWriteEnable(15);

/// The header's `D3D12_COMMAND_LIST_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandListFlags(pub i32);

/// The header's `D3D12_COMMAND_LIST_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_flags).
pub const D3D12_COMMAND_LIST_FLAG_NONE: CommandListFlags = CommandListFlags(0);

/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandListSupportFlags(pub i32);

/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
pub const D3D12_COMMAND_LIST_SUPPORT_FLAG_NONE: CommandListSupportFlags =
    CommandListSupportFlags(0);
/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAG_DIRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
pub const D3D12_COMMAND_LIST_SUPPORT_FLAG_DIRECT: CommandListSupportFlags =
    CommandListSupportFlags(1);
/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAG_BUNDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
pub const D3D12_COMMAND_LIST_SUPPORT_FLAG_BUNDLE: CommandListSupportFlags =
    CommandListSupportFlags(2);
/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAG_COMPUTE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
pub const D3D12_COMMAND_LIST_SUPPORT_FLAG_COMPUTE: CommandListSupportFlags =
    CommandListSupportFlags(4);
/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAG_COPY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
pub const D3D12_COMMAND_LIST_SUPPORT_FLAG_COPY: CommandListSupportFlags =
    CommandListSupportFlags(8);
/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAG_VIDEO_DECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
pub const D3D12_COMMAND_LIST_SUPPORT_FLAG_VIDEO_DECODE: CommandListSupportFlags =
    CommandListSupportFlags(16);
/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAG_VIDEO_PROCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
pub const D3D12_COMMAND_LIST_SUPPORT_FLAG_VIDEO_PROCESS: CommandListSupportFlags =
    CommandListSupportFlags(32);
/// The header's `D3D12_COMMAND_LIST_SUPPORT_FLAG_VIDEO_ENCODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_support_flags).
pub const D3D12_COMMAND_LIST_SUPPORT_FLAG_VIDEO_ENCODE: CommandListSupportFlags =
    CommandListSupportFlags(64);

/// The header's `D3D12_COMMAND_LIST_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandListType(pub i32);

/// The header's `D3D12_COMMAND_LIST_TYPE_DIRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
pub const D3D12_COMMAND_LIST_TYPE_DIRECT: CommandListType = CommandListType(0);
/// The header's `D3D12_COMMAND_LIST_TYPE_BUNDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
pub const D3D12_COMMAND_LIST_TYPE_BUNDLE: CommandListType = CommandListType(1);
/// The header's `D3D12_COMMAND_LIST_TYPE_COMPUTE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
pub const D3D12_COMMAND_LIST_TYPE_COMPUTE: CommandListType = CommandListType(2);
/// The header's `D3D12_COMMAND_LIST_TYPE_COPY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
pub const D3D12_COMMAND_LIST_TYPE_COPY: CommandListType = CommandListType(3);
/// The header's `D3D12_COMMAND_LIST_TYPE_VIDEO_DECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
pub const D3D12_COMMAND_LIST_TYPE_VIDEO_DECODE: CommandListType = CommandListType(4);
/// The header's `D3D12_COMMAND_LIST_TYPE_VIDEO_PROCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
pub const D3D12_COMMAND_LIST_TYPE_VIDEO_PROCESS: CommandListType = CommandListType(5);
/// The header's `D3D12_COMMAND_LIST_TYPE_VIDEO_ENCODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
pub const D3D12_COMMAND_LIST_TYPE_VIDEO_ENCODE: CommandListType = CommandListType(6);
/// The header's `D3D12_COMMAND_LIST_TYPE_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_list_type).
pub const D3D12_COMMAND_LIST_TYPE_NONE: CommandListType = CommandListType(-1);

/// The header's `D3D12_COMMAND_POOL_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandPoolFlags(pub i32);

/// The header's `D3D12_COMMAND_POOL_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_COMMAND_POOL_FLAG_NONE: CommandPoolFlags = CommandPoolFlags(0);

/// The header's `D3D12_COMMAND_QUEUE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_queue_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueFlags(pub i32);

/// The header's `D3D12_COMMAND_QUEUE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_queue_flags).
pub const D3D12_COMMAND_QUEUE_FLAG_NONE: CommandQueueFlags = CommandQueueFlags(0);
/// The header's `D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_queue_flags).
pub const D3D12_COMMAND_QUEUE_FLAG_DISABLE_GPU_TIMEOUT: CommandQueueFlags = CommandQueueFlags(1);

/// The header's `D3D12_COMMAND_QUEUE_PRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_queue_priority).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandQueuePriority(pub i32);

/// The header's `D3D12_COMMAND_QUEUE_PRIORITY_NORMAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_queue_priority).
pub const D3D12_COMMAND_QUEUE_PRIORITY_NORMAL: CommandQueuePriority = CommandQueuePriority(0);
/// The header's `D3D12_COMMAND_QUEUE_PRIORITY_HIGH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_queue_priority).
pub const D3D12_COMMAND_QUEUE_PRIORITY_HIGH: CommandQueuePriority = CommandQueuePriority(100);
/// The header's `D3D12_COMMAND_QUEUE_PRIORITY_GLOBAL_REALTIME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_command_queue_priority).
pub const D3D12_COMMAND_QUEUE_PRIORITY_GLOBAL_REALTIME: CommandQueuePriority =
    CommandQueuePriority(0x2710);

/// The header's `D3D12_COMMAND_RECORDER_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandRecorderFlags(pub i32);

/// The header's `D3D12_COMMAND_RECORDER_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_COMMAND_RECORDER_FLAG_NONE: CommandRecorderFlags = CommandRecorderFlags(0);

/// The header's `D3D12_COMPARISON_FUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ComparisonFunc(pub i32);

/// The header's `D3D12_COMPARISON_FUNC_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_NONE: ComparisonFunc = ComparisonFunc(0);
/// The header's `D3D12_COMPARISON_FUNC_NEVER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_NEVER: ComparisonFunc = ComparisonFunc(1);
/// The header's `D3D12_COMPARISON_FUNC_LESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_LESS: ComparisonFunc = ComparisonFunc(2);
/// The header's `D3D12_COMPARISON_FUNC_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_EQUAL: ComparisonFunc = ComparisonFunc(3);
/// The header's `D3D12_COMPARISON_FUNC_LESS_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_LESS_EQUAL: ComparisonFunc = ComparisonFunc(4);
/// The header's `D3D12_COMPARISON_FUNC_GREATER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_GREATER: ComparisonFunc = ComparisonFunc(5);
/// The header's `D3D12_COMPARISON_FUNC_NOT_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_NOT_EQUAL: ComparisonFunc = ComparisonFunc(6);
/// The header's `D3D12_COMPARISON_FUNC_GREATER_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_GREATER_EQUAL: ComparisonFunc = ComparisonFunc(7);
/// The header's `D3D12_COMPARISON_FUNC_ALWAYS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_comparison_func).
pub const D3D12_COMPARISON_FUNC_ALWAYS: ComparisonFunc = ComparisonFunc(8);

/// The header's `D3D12_CONSERVATIVE_RASTERIZATION_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_conservative_rasterization_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConservativeRasterizationMode(pub i32);

/// The header's `D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_conservative_rasterization_mode).
pub const D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF: ConservativeRasterizationMode =
    ConservativeRasterizationMode(0);
/// The header's `D3D12_CONSERVATIVE_RASTERIZATION_MODE_ON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_conservative_rasterization_mode).
pub const D3D12_CONSERVATIVE_RASTERIZATION_MODE_ON: ConservativeRasterizationMode =
    ConservativeRasterizationMode(1);

/// The header's `D3D12_CONSERVATIVE_RASTERIZATION_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_conservative_rasterization_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConservativeRasterizationTier(pub i32);

/// The header's `D3D12_CONSERVATIVE_RASTERIZATION_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_conservative_rasterization_tier).
pub const D3D12_CONSERVATIVE_RASTERIZATION_TIER_NOT_SUPPORTED: ConservativeRasterizationTier =
    ConservativeRasterizationTier(0);
/// The header's `D3D12_CONSERVATIVE_RASTERIZATION_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_conservative_rasterization_tier).
pub const D3D12_CONSERVATIVE_RASTERIZATION_TIER_1: ConservativeRasterizationTier =
    ConservativeRasterizationTier(1);
/// The header's `D3D12_CONSERVATIVE_RASTERIZATION_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_conservative_rasterization_tier).
pub const D3D12_CONSERVATIVE_RASTERIZATION_TIER_2: ConservativeRasterizationTier =
    ConservativeRasterizationTier(2);
/// The header's `D3D12_CONSERVATIVE_RASTERIZATION_TIER_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_conservative_rasterization_tier).
pub const D3D12_CONSERVATIVE_RASTERIZATION_TIER_3: ConservativeRasterizationTier =
    ConservativeRasterizationTier(3);

/// The header's `D3D12_CPU_PAGE_PROPERTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cpu_page_property).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CpuPageProperty(pub i32);

/// The header's `D3D12_CPU_PAGE_PROPERTY_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cpu_page_property).
pub const D3D12_CPU_PAGE_PROPERTY_UNKNOWN: CpuPageProperty = CpuPageProperty(0);
/// The header's `D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cpu_page_property).
pub const D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE: CpuPageProperty = CpuPageProperty(1);
/// The header's `D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cpu_page_property).
pub const D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE: CpuPageProperty = CpuPageProperty(2);
/// The header's `D3D12_CPU_PAGE_PROPERTY_WRITE_BACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cpu_page_property).
pub const D3D12_CPU_PAGE_PROPERTY_WRITE_BACK: CpuPageProperty = CpuPageProperty(3);

/// The header's `D3D12_CROSS_NODE_SHARING_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cross_node_sharing_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CrossNodeSharingTier(pub i32);

/// The header's `D3D12_CROSS_NODE_SHARING_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cross_node_sharing_tier).
pub const D3D12_CROSS_NODE_SHARING_TIER_NOT_SUPPORTED: CrossNodeSharingTier =
    CrossNodeSharingTier(0);
/// The header's `D3D12_CROSS_NODE_SHARING_TIER_1_EMULATED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cross_node_sharing_tier).
pub const D3D12_CROSS_NODE_SHARING_TIER_1_EMULATED: CrossNodeSharingTier = CrossNodeSharingTier(1);
/// The header's `D3D12_CROSS_NODE_SHARING_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cross_node_sharing_tier).
pub const D3D12_CROSS_NODE_SHARING_TIER_1: CrossNodeSharingTier = CrossNodeSharingTier(2);
/// The header's `D3D12_CROSS_NODE_SHARING_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cross_node_sharing_tier).
pub const D3D12_CROSS_NODE_SHARING_TIER_2: CrossNodeSharingTier = CrossNodeSharingTier(3);
/// The header's `D3D12_CROSS_NODE_SHARING_TIER_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cross_node_sharing_tier).
pub const D3D12_CROSS_NODE_SHARING_TIER_3: CrossNodeSharingTier = CrossNodeSharingTier(4);

/// The header's `D3D12_CULL_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cull_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CullMode(pub i32);

/// The header's `D3D12_CULL_MODE_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cull_mode).
pub const D3D12_CULL_MODE_NONE: CullMode = CullMode(1);
/// The header's `D3D12_CULL_MODE_FRONT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cull_mode).
pub const D3D12_CULL_MODE_FRONT: CullMode = CullMode(2);
/// The header's `D3D12_CULL_MODE_BACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_cull_mode).
pub const D3D12_CULL_MODE_BACK: CullMode = CullMode(3);

/// The header's `D3D12_DEBUG_COMMAND_LIST_PARAMETER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_command_list_parameter_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DebugCommandListParameterType(pub i32);

/// The header's `D3D12_DEBUG_COMMAND_LIST_PARAMETER_GPU_BASED_VALIDATION_SETTINGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_command_list_parameter_type).
pub const D3D12_DEBUG_COMMAND_LIST_PARAMETER_GPU_BASED_VALIDATION_SETTINGS:
    DebugCommandListParameterType = DebugCommandListParameterType(0);

/// The header's `D3D12_DEBUG_DEVICE_PARAMETER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_device_parameter_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DebugDeviceParameterType(pub i32);

/// The header's `D3D12_DEBUG_DEVICE_PARAMETER_FEATURE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_device_parameter_type).
pub const D3D12_DEBUG_DEVICE_PARAMETER_FEATURE_FLAGS: DebugDeviceParameterType =
    DebugDeviceParameterType(0);
/// The header's `D3D12_DEBUG_DEVICE_PARAMETER_GPU_BASED_VALIDATION_SETTINGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_device_parameter_type).
pub const D3D12_DEBUG_DEVICE_PARAMETER_GPU_BASED_VALIDATION_SETTINGS: DebugDeviceParameterType =
    DebugDeviceParameterType(1);
/// The header's `D3D12_DEBUG_DEVICE_PARAMETER_GPU_SLOWDOWN_PERFORMANCE_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_device_parameter_type).
pub const D3D12_DEBUG_DEVICE_PARAMETER_GPU_SLOWDOWN_PERFORMANCE_FACTOR: DebugDeviceParameterType =
    DebugDeviceParameterType(2);

/// The header's `D3D12_DEBUG_FEATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_feature).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DebugFeature(pub i32);

/// The header's `D3D12_DEBUG_FEATURE_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_feature).
pub const D3D12_DEBUG_FEATURE_NONE: DebugFeature = DebugFeature(0);
/// The header's `D3D12_DEBUG_FEATURE_ALLOW_BEHAVIOR_CHANGING_DEBUG_AIDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_feature).
pub const D3D12_DEBUG_FEATURE_ALLOW_BEHAVIOR_CHANGING_DEBUG_AIDS: DebugFeature = DebugFeature(1);
/// The header's `D3D12_DEBUG_FEATURE_CONSERVATIVE_RESOURCE_STATE_TRACKING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_feature).
pub const D3D12_DEBUG_FEATURE_CONSERVATIVE_RESOURCE_STATE_TRACKING: DebugFeature = DebugFeature(2);
/// The header's `D3D12_DEBUG_FEATURE_DISABLE_VIRTUALIZED_BUNDLES_VALIDATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_feature).
pub const D3D12_DEBUG_FEATURE_DISABLE_VIRTUALIZED_BUNDLES_VALIDATION: DebugFeature =
    DebugFeature(4);
/// The header's `D3D12_DEBUG_FEATURE_EMULATE_WINDOWS7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_debug_feature).
pub const D3D12_DEBUG_FEATURE_EMULATE_WINDOWS7: DebugFeature = DebugFeature(8);

/// The header's `D3D12_DEPTH_WRITE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_depth_write_mask).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DepthWriteMask(pub i32);

/// The header's `D3D12_DEPTH_WRITE_MASK_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_depth_write_mask).
pub const D3D12_DEPTH_WRITE_MASK_ZERO: DepthWriteMask = DepthWriteMask(0);
/// The header's `D3D12_DEPTH_WRITE_MASK_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_depth_write_mask).
pub const D3D12_DEPTH_WRITE_MASK_ALL: DepthWriteMask = DepthWriteMask(1);

/// The header's `D3D12_DESCRIPTOR_HEAP_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DescriptorHeapFlags(pub i32);

/// The header's `D3D12_DESCRIPTOR_HEAP_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_flags).
pub const D3D12_DESCRIPTOR_HEAP_FLAG_NONE: DescriptorHeapFlags = DescriptorHeapFlags(0);
/// The header's `D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_flags).
pub const D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE: DescriptorHeapFlags = DescriptorHeapFlags(1);

/// The header's `D3D12_DESCRIPTOR_HEAP_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DescriptorHeapType(pub i32);

/// The header's `D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_type).
pub const D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV: DescriptorHeapType = DescriptorHeapType(0);
/// The header's `D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_type).
pub const D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER: DescriptorHeapType = DescriptorHeapType(1);
/// The header's `D3D12_DESCRIPTOR_HEAP_TYPE_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_type).
pub const D3D12_DESCRIPTOR_HEAP_TYPE_RTV: DescriptorHeapType = DescriptorHeapType(2);
/// The header's `D3D12_DESCRIPTOR_HEAP_TYPE_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_type).
pub const D3D12_DESCRIPTOR_HEAP_TYPE_DSV: DescriptorHeapType = DescriptorHeapType(3);
/// The header's `D3D12_DESCRIPTOR_HEAP_TYPE_NUM_TYPES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_heap_type).
pub const D3D12_DESCRIPTOR_HEAP_TYPE_NUM_TYPES: DescriptorHeapType = DescriptorHeapType(4);

/// The header's `D3D12_DESCRIPTOR_RANGE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DescriptorRangeFlags(pub i32);

/// The header's `D3D12_DESCRIPTOR_RANGE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_flags).
pub const D3D12_DESCRIPTOR_RANGE_FLAG_NONE: DescriptorRangeFlags = DescriptorRangeFlags(0);
/// The header's `D3D12_DESCRIPTOR_RANGE_FLAG_DESCRIPTORS_VOLATILE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_flags).
pub const D3D12_DESCRIPTOR_RANGE_FLAG_DESCRIPTORS_VOLATILE: DescriptorRangeFlags =
    DescriptorRangeFlags(1);
/// The header's `D3D12_DESCRIPTOR_RANGE_FLAG_DATA_VOLATILE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_flags).
pub const D3D12_DESCRIPTOR_RANGE_FLAG_DATA_VOLATILE: DescriptorRangeFlags = DescriptorRangeFlags(2);
/// The header's `D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC_WHILE_SET_AT_EXECUTE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_flags).
pub const D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC_WHILE_SET_AT_EXECUTE: DescriptorRangeFlags =
    DescriptorRangeFlags(4);
/// The header's `D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_flags).
pub const D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC: DescriptorRangeFlags = DescriptorRangeFlags(8);
/// The header's `D3D12_DESCRIPTOR_RANGE_FLAG_DESCRIPTORS_STATIC_KEEPING_BUFFER_BOUNDS_CHECKS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_flags).
pub const D3D12_DESCRIPTOR_RANGE_FLAG_DESCRIPTORS_STATIC_KEEPING_BUFFER_BOUNDS_CHECKS:
    DescriptorRangeFlags = DescriptorRangeFlags(0x10000);

/// The header's `D3D12_DESCRIPTOR_RANGE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DescriptorRangeType(pub i32);

/// The header's `D3D12_DESCRIPTOR_RANGE_TYPE_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_type).
pub const D3D12_DESCRIPTOR_RANGE_TYPE_SRV: DescriptorRangeType = DescriptorRangeType(0);
/// The header's `D3D12_DESCRIPTOR_RANGE_TYPE_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_type).
pub const D3D12_DESCRIPTOR_RANGE_TYPE_UAV: DescriptorRangeType = DescriptorRangeType(1);
/// The header's `D3D12_DESCRIPTOR_RANGE_TYPE_CBV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_type).
pub const D3D12_DESCRIPTOR_RANGE_TYPE_CBV: DescriptorRangeType = DescriptorRangeType(2);
/// The header's `D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_descriptor_range_type).
pub const D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER: DescriptorRangeType = DescriptorRangeType(3);

/// The header's `D3D12_DEVICE_FACTORY_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeviceFactoryFlags(pub i32);

/// The header's `D3D12_DEVICE_FACTORY_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FACTORY_FLAG_NONE: DeviceFactoryFlags = DeviceFactoryFlags(0);
/// The header's `D3D12_DEVICE_FACTORY_FLAG_ALLOW_RETURNING_EXISTING_DEVICE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FACTORY_FLAG_ALLOW_RETURNING_EXISTING_DEVICE: DeviceFactoryFlags =
    DeviceFactoryFlags(1);
/// The header's `D3D12_DEVICE_FACTORY_FLAG_ALLOW_RETURNING_INCOMPATIBLE_EXISTING_DEVICE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FACTORY_FLAG_ALLOW_RETURNING_INCOMPATIBLE_EXISTING_DEVICE:
    DeviceFactoryFlags = DeviceFactoryFlags(2);
/// The header's `D3D12_DEVICE_FACTORY_FLAG_DISALLOW_STORING_NEW_DEVICE_AS_SINGLETON`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FACTORY_FLAG_DISALLOW_STORING_NEW_DEVICE_AS_SINGLETON: DeviceFactoryFlags =
    DeviceFactoryFlags(4);

/// The header's `D3D12_DEVICE_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeviceFlags(pub i32);

/// The header's `D3D12_DEVICE_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_NONE: DeviceFlags = DeviceFlags(0);
/// The header's `D3D12_DEVICE_FLAG_DEBUG_LAYER_ENABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_DEBUG_LAYER_ENABLED: DeviceFlags = DeviceFlags(1);
/// The header's `D3D12_DEVICE_FLAG_GPU_BASED_VALIDATION_ENABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_GPU_BASED_VALIDATION_ENABLED: DeviceFlags = DeviceFlags(2);
/// The header's `D3D12_DEVICE_FLAG_SYNCHRONIZED_COMMAND_QUEUE_VALIDATION_DISABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_SYNCHRONIZED_COMMAND_QUEUE_VALIDATION_DISABLED: DeviceFlags =
    DeviceFlags(4);
/// The header's `D3D12_DEVICE_FLAG_DRED_AUTO_BREADCRUMBS_ENABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_DRED_AUTO_BREADCRUMBS_ENABLED: DeviceFlags = DeviceFlags(8);
/// The header's `D3D12_DEVICE_FLAG_DRED_PAGE_FAULT_REPORTING_ENABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_DRED_PAGE_FAULT_REPORTING_ENABLED: DeviceFlags = DeviceFlags(16);
/// The header's `D3D12_DEVICE_FLAG_DRED_WATSON_REPORTING_ENABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_DRED_WATSON_REPORTING_ENABLED: DeviceFlags = DeviceFlags(32);
/// The header's `D3D12_DEVICE_FLAG_DRED_BREADCRUMB_CONTEXT_ENABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_DRED_BREADCRUMB_CONTEXT_ENABLED: DeviceFlags = DeviceFlags(64);
/// The header's `D3D12_DEVICE_FLAG_DRED_USE_MARKERS_ONLY_BREADCRUMBS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_DRED_USE_MARKERS_ONLY_BREADCRUMBS: DeviceFlags = DeviceFlags(128);
/// The header's `D3D12_DEVICE_FLAG_SHADER_INSTRUMENTATION_ENABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_SHADER_INSTRUMENTATION_ENABLED: DeviceFlags = DeviceFlags(0x100);
/// The header's `D3D12_DEVICE_FLAG_AUTO_DEBUG_NAME_ENABLED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_AUTO_DEBUG_NAME_ENABLED: DeviceFlags = DeviceFlags(0x200);
/// The header's `D3D12_DEVICE_FLAG_FORCE_LEGACY_STATE_VALIDATION`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DEVICE_FLAG_FORCE_LEGACY_STATE_VALIDATION: DeviceFlags = DeviceFlags(0x400);

/// The header's `D3D12_DISPATCH_MODE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DispatchMode(pub i32);

/// The header's `D3D12_DISPATCH_MODE_NODE_CPU_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DISPATCH_MODE_NODE_CPU_INPUT: DispatchMode = DispatchMode(0);
/// The header's `D3D12_DISPATCH_MODE_NODE_GPU_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DISPATCH_MODE_NODE_GPU_INPUT: DispatchMode = DispatchMode(1);
/// The header's `D3D12_DISPATCH_MODE_MULTI_NODE_CPU_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DISPATCH_MODE_MULTI_NODE_CPU_INPUT: DispatchMode = DispatchMode(2);
/// The header's `D3D12_DISPATCH_MODE_MULTI_NODE_GPU_INPUT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DISPATCH_MODE_MULTI_NODE_GPU_INPUT: DispatchMode = DispatchMode(3);

/// The header's `D3D12_DRED_ALLOCATION_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DredAllocationType(pub i32);

/// The header's `D3D12_DRED_ALLOCATION_TYPE_COMMAND_QUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_COMMAND_QUEUE: DredAllocationType = DredAllocationType(19);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_COMMAND_ALLOCATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_COMMAND_ALLOCATOR: DredAllocationType = DredAllocationType(20);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_PIPELINE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_PIPELINE_STATE: DredAllocationType = DredAllocationType(21);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_COMMAND_LIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_COMMAND_LIST: DredAllocationType = DredAllocationType(22);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_FENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_FENCE: DredAllocationType = DredAllocationType(23);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_DESCRIPTOR_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_DESCRIPTOR_HEAP: DredAllocationType = DredAllocationType(24);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_HEAP: DredAllocationType = DredAllocationType(25);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_QUERY_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_QUERY_HEAP: DredAllocationType = DredAllocationType(27);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_COMMAND_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_COMMAND_SIGNATURE: DredAllocationType = DredAllocationType(28);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_PIPELINE_LIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_PIPELINE_LIBRARY: DredAllocationType = DredAllocationType(29);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_VIDEO_DECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_VIDEO_DECODER: DredAllocationType = DredAllocationType(30);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_VIDEO_PROCESSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_VIDEO_PROCESSOR: DredAllocationType = DredAllocationType(32);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_RESOURCE: DredAllocationType = DredAllocationType(34);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_PASS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_PASS: DredAllocationType = DredAllocationType(35);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_CRYPTOSESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_CRYPTOSESSION: DredAllocationType = DredAllocationType(36);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_CRYPTOSESSIONPOLICY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_CRYPTOSESSIONPOLICY: DredAllocationType =
    DredAllocationType(37);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_PROTECTEDRESOURCESESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_PROTECTEDRESOURCESESSION: DredAllocationType =
    DredAllocationType(38);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_VIDEO_DECODER_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_VIDEO_DECODER_HEAP: DredAllocationType =
    DredAllocationType(39);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_COMMAND_POOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_COMMAND_POOL: DredAllocationType = DredAllocationType(40);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_COMMAND_RECORDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_COMMAND_RECORDER: DredAllocationType = DredAllocationType(41);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_STATE_OBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_STATE_OBJECT: DredAllocationType = DredAllocationType(42);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_METACOMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_METACOMMAND: DredAllocationType = DredAllocationType(43);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_SCHEDULINGGROUP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_SCHEDULINGGROUP: DredAllocationType = DredAllocationType(44);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_VIDEO_MOTION_ESTIMATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_VIDEO_MOTION_ESTIMATOR: DredAllocationType =
    DredAllocationType(45);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_VIDEO_MOTION_VECTOR_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_VIDEO_MOTION_VECTOR_HEAP: DredAllocationType =
    DredAllocationType(46);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_VIDEO_EXTENSION_COMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_VIDEO_EXTENSION_COMMAND: DredAllocationType =
    DredAllocationType(47);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_VIDEO_ENCODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_VIDEO_ENCODER: DredAllocationType = DredAllocationType(48);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_VIDEO_ENCODER_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_VIDEO_ENCODER_HEAP: DredAllocationType =
    DredAllocationType(49);
/// The header's `D3D12_DRED_ALLOCATION_TYPE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_allocation_type).
pub const D3D12_DRED_ALLOCATION_TYPE_INVALID: DredAllocationType = DredAllocationType(-1);

/// The header's `D3D12_DRED_DEVICE_STATE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DredDeviceState(pub i32);

/// The header's `D3D12_DRED_DEVICE_STATE_UNKNOWN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DRED_DEVICE_STATE_UNKNOWN: DredDeviceState = DredDeviceState(0);
/// The header's `D3D12_DRED_DEVICE_STATE_HUNG`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DRED_DEVICE_STATE_HUNG: DredDeviceState = DredDeviceState(3);
/// The header's `D3D12_DRED_DEVICE_STATE_FAULT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DRED_DEVICE_STATE_FAULT: DredDeviceState = DredDeviceState(6);
/// The header's `D3D12_DRED_DEVICE_STATE_PAGEFAULT`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DRED_DEVICE_STATE_PAGEFAULT: DredDeviceState = DredDeviceState(7);

/// The header's `D3D12_DRED_ENABLEMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_enablement).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DredEnablement(pub i32);

/// The header's `D3D12_DRED_ENABLEMENT_SYSTEM_CONTROLLED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_enablement).
pub const D3D12_DRED_ENABLEMENT_SYSTEM_CONTROLLED: DredEnablement = DredEnablement(0);
/// The header's `D3D12_DRED_ENABLEMENT_FORCED_OFF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_enablement).
pub const D3D12_DRED_ENABLEMENT_FORCED_OFF: DredEnablement = DredEnablement(1);
/// The header's `D3D12_DRED_ENABLEMENT_FORCED_ON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_enablement).
pub const D3D12_DRED_ENABLEMENT_FORCED_ON: DredEnablement = DredEnablement(2);

/// The header's `D3D12_DRED_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DredFlags(pub i32);

/// The header's `D3D12_DRED_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_flags).
pub const D3D12_DRED_FLAG_NONE: DredFlags = DredFlags(0);
/// The header's `D3D12_DRED_FLAG_FORCE_ENABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_flags).
pub const D3D12_DRED_FLAG_FORCE_ENABLE: DredFlags = DredFlags(1);
/// The header's `D3D12_DRED_FLAG_DISABLE_AUTOBREADCRUMBS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_flags).
pub const D3D12_DRED_FLAG_DISABLE_AUTOBREADCRUMBS: DredFlags = DredFlags(2);

/// The header's `D3D12_DRED_PAGE_FAULT_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DredPageFaultFlags(pub i32);

/// The header's `D3D12_DRED_PAGE_FAULT_FLAGS_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_DRED_PAGE_FAULT_FLAGS_NONE: DredPageFaultFlags = DredPageFaultFlags(0);

/// The header's `D3D12_DRED_VERSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_version).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DredVersion(pub i32);

/// The header's `D3D12_DRED_VERSION_1_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_version).
pub const D3D12_DRED_VERSION_1_0: DredVersion = DredVersion(1);
/// The header's `D3D12_DRED_VERSION_1_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_version).
pub const D3D12_DRED_VERSION_1_1: DredVersion = DredVersion(2);
/// The header's `D3D12_DRED_VERSION_1_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_version).
pub const D3D12_DRED_VERSION_1_2: DredVersion = DredVersion(3);
/// The header's `D3D12_DRED_VERSION_1_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dred_version).
pub const D3D12_DRED_VERSION_1_3: DredVersion = DredVersion(4);

/// The header's `D3D12_DRIVER_MATCHING_IDENTIFIER_STATUS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_driver_matching_identifier_status).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DriverMatchingIdentifierStatus(pub i32);

/// The header's `D3D12_DRIVER_MATCHING_IDENTIFIER_COMPATIBLE_WITH_DEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_driver_matching_identifier_status).
pub const D3D12_DRIVER_MATCHING_IDENTIFIER_COMPATIBLE_WITH_DEVICE: DriverMatchingIdentifierStatus =
    DriverMatchingIdentifierStatus(0);
/// The header's `D3D12_DRIVER_MATCHING_IDENTIFIER_UNSUPPORTED_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_driver_matching_identifier_status).
pub const D3D12_DRIVER_MATCHING_IDENTIFIER_UNSUPPORTED_TYPE: DriverMatchingIdentifierStatus =
    DriverMatchingIdentifierStatus(1);
/// The header's `D3D12_DRIVER_MATCHING_IDENTIFIER_UNRECOGNIZED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_driver_matching_identifier_status).
pub const D3D12_DRIVER_MATCHING_IDENTIFIER_UNRECOGNIZED: DriverMatchingIdentifierStatus =
    DriverMatchingIdentifierStatus(2);
/// The header's `D3D12_DRIVER_MATCHING_IDENTIFIER_INCOMPATIBLE_VERSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_driver_matching_identifier_status).
pub const D3D12_DRIVER_MATCHING_IDENTIFIER_INCOMPATIBLE_VERSION: DriverMatchingIdentifierStatus =
    DriverMatchingIdentifierStatus(3);
/// The header's `D3D12_DRIVER_MATCHING_IDENTIFIER_INCOMPATIBLE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_driver_matching_identifier_status).
pub const D3D12_DRIVER_MATCHING_IDENTIFIER_INCOMPATIBLE_TYPE: DriverMatchingIdentifierStatus =
    DriverMatchingIdentifierStatus(4);

/// The header's `D3D12_DSV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DsvDimension(pub i32);

/// The header's `D3D12_DSV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_dimension).
pub const D3D12_DSV_DIMENSION_UNKNOWN: DsvDimension = DsvDimension(0);
/// The header's `D3D12_DSV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_dimension).
pub const D3D12_DSV_DIMENSION_TEXTURE1D: DsvDimension = DsvDimension(1);
/// The header's `D3D12_DSV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_dimension).
pub const D3D12_DSV_DIMENSION_TEXTURE1DARRAY: DsvDimension = DsvDimension(2);
/// The header's `D3D12_DSV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_dimension).
pub const D3D12_DSV_DIMENSION_TEXTURE2D: DsvDimension = DsvDimension(3);
/// The header's `D3D12_DSV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_dimension).
pub const D3D12_DSV_DIMENSION_TEXTURE2DARRAY: DsvDimension = DsvDimension(4);
/// The header's `D3D12_DSV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_dimension).
pub const D3D12_DSV_DIMENSION_TEXTURE2DMS: DsvDimension = DsvDimension(5);
/// The header's `D3D12_DSV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_dimension).
pub const D3D12_DSV_DIMENSION_TEXTURE2DMSARRAY: DsvDimension = DsvDimension(6);

/// The header's `D3D12_DSV_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DsvFlags(pub i32);

/// The header's `D3D12_DSV_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_flags).
pub const D3D12_DSV_FLAG_NONE: DsvFlags = DsvFlags(0);
/// The header's `D3D12_DSV_FLAG_READ_ONLY_DEPTH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_flags).
pub const D3D12_DSV_FLAG_READ_ONLY_DEPTH: DsvFlags = DsvFlags(1);
/// The header's `D3D12_DSV_FLAG_READ_ONLY_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_dsv_flags).
pub const D3D12_DSV_FLAG_READ_ONLY_STENCIL: DsvFlags = DsvFlags(2);

/// The header's `D3D12_ELEMENTS_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_elements_layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ElementsLayout(pub i32);

/// The header's `D3D12_ELEMENTS_LAYOUT_ARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_elements_layout).
pub const D3D12_ELEMENTS_LAYOUT_ARRAY: ElementsLayout = ElementsLayout(0);
/// The header's `D3D12_ELEMENTS_LAYOUT_ARRAY_OF_POINTERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_elements_layout).
pub const D3D12_ELEMENTS_LAYOUT_ARRAY_OF_POINTERS: ElementsLayout = ElementsLayout(1);

/// The header's `D3D12_EXECUTE_INDIRECT_TIER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExecuteIndirectTier(pub i32);

/// The header's `D3D12_EXECUTE_INDIRECT_TIER_1_0`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_EXECUTE_INDIRECT_TIER_1_0: ExecuteIndirectTier = ExecuteIndirectTier(10);
/// The header's `D3D12_EXECUTE_INDIRECT_TIER_1_1`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_EXECUTE_INDIRECT_TIER_1_1: ExecuteIndirectTier = ExecuteIndirectTier(11);

/// The header's `D3D12_EXPORT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_export_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExportFlags(pub i32);

/// The header's `D3D12_EXPORT_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_export_flags).
pub const D3D12_EXPORT_FLAG_NONE: ExportFlags = ExportFlags(0);

/// The header's `D3D12_FEATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Feature(pub i32);

/// The header's `D3D12_FEATURE_D3D12_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS: Feature = Feature(0);
/// The header's `D3D12_FEATURE_ARCHITECTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_ARCHITECTURE: Feature = Feature(1);
/// The header's `D3D12_FEATURE_FEATURE_LEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_FEATURE_LEVELS: Feature = Feature(2);
/// The header's `D3D12_FEATURE_FORMAT_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_FORMAT_SUPPORT: Feature = Feature(3);
/// The header's `D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS: Feature = Feature(4);
/// The header's `D3D12_FEATURE_FORMAT_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_FORMAT_INFO: Feature = Feature(5);
/// The header's `D3D12_FEATURE_GPU_VIRTUAL_ADDRESS_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_GPU_VIRTUAL_ADDRESS_SUPPORT: Feature = Feature(6);
/// The header's `D3D12_FEATURE_SHADER_MODEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_SHADER_MODEL: Feature = Feature(7);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS1: Feature = Feature(8);
/// The header's `D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_SUPPORT: Feature = Feature(10);
/// The header's `D3D12_FEATURE_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_ROOT_SIGNATURE: Feature = Feature(12);
/// The header's `D3D12_FEATURE_ARCHITECTURE1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_ARCHITECTURE1: Feature = Feature(16);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS2: Feature = Feature(18);
/// The header's `D3D12_FEATURE_SHADER_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_SHADER_CACHE: Feature = Feature(19);
/// The header's `D3D12_FEATURE_COMMAND_QUEUE_PRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_COMMAND_QUEUE_PRIORITY: Feature = Feature(20);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS3: Feature = Feature(21);
/// The header's `D3D12_FEATURE_EXISTING_HEAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_EXISTING_HEAPS: Feature = Feature(22);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS4: Feature = Feature(23);
/// The header's `D3D12_FEATURE_SERIALIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_SERIALIZATION: Feature = Feature(24);
/// The header's `D3D12_FEATURE_CROSS_NODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_CROSS_NODE: Feature = Feature(25);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS5: Feature = Feature(27);
/// The header's `D3D12_FEATURE_DISPLAYABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_DISPLAYABLE: Feature = Feature(28);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS6: Feature = Feature(30);
/// The header's `D3D12_FEATURE_QUERY_META_COMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_QUERY_META_COMMAND: Feature = Feature(31);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS7: Feature = Feature(32);
/// The header's `D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_TYPE_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_TYPE_COUNT: Feature = Feature(33);
/// The header's `D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_TYPES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_TYPES: Feature = Feature(34);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS8: Feature = Feature(36);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS9`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS9: Feature = Feature(37);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS10: Feature = Feature(39);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS11`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS11: Feature = Feature(40);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS12`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS12: Feature = Feature(41);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS13`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS13: Feature = Feature(42);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS14`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS14: Feature = Feature(43);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS15`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS15: Feature = Feature(44);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS16: Feature = Feature(45);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS17`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS17: Feature = Feature(46);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS18`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS18: Feature = Feature(47);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS19`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS19: Feature = Feature(48);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS20`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS20: Feature = Feature(49);
/// The header's `D3D12_FEATURE_PREDICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_PREDICATION: Feature = Feature(50);
/// The header's `D3D12_FEATURE_PLACED_RESOURCE_SUPPORT_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_PLACED_RESOURCE_SUPPORT_INFO: Feature = Feature(51);
/// The header's `D3D12_FEATURE_HARDWARE_COPY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_HARDWARE_COPY: Feature = Feature(52);
/// The header's `D3D12_FEATURE_D3D12_OPTIONS21`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_feature).
pub const D3D12_FEATURE_D3D12_OPTIONS21: Feature = Feature(53);

/// The header's `D3D12_FENCE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_fence_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FenceFlags(pub i32);

/// The header's `D3D12_FENCE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_fence_flags).
pub const D3D12_FENCE_FLAG_NONE: FenceFlags = FenceFlags(0);
/// The header's `D3D12_FENCE_FLAG_SHARED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_fence_flags).
pub const D3D12_FENCE_FLAG_SHARED: FenceFlags = FenceFlags(1);
/// The header's `D3D12_FENCE_FLAG_SHARED_CROSS_ADAPTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_fence_flags).
pub const D3D12_FENCE_FLAG_SHARED_CROSS_ADAPTER: FenceFlags = FenceFlags(2);
/// The header's `D3D12_FENCE_FLAG_NON_MONITORED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_fence_flags).
pub const D3D12_FENCE_FLAG_NON_MONITORED: FenceFlags = FenceFlags(4);

/// The header's `D3D12_FILL_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_fill_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FillMode(pub i32);

/// The header's `D3D12_FILL_MODE_WIREFRAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_fill_mode).
pub const D3D12_FILL_MODE_WIREFRAME: FillMode = FillMode(2);
/// The header's `D3D12_FILL_MODE_SOLID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_fill_mode).
pub const D3D12_FILL_MODE_SOLID: FillMode = FillMode(3);

/// The header's `D3D12_FILTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Filter(pub i32);

/// The header's `D3D12_FILTER_MIN_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_MAG_MIP_POINT: Filter = Filter(0);
/// The header's `D3D12_FILTER_MIN_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_MAG_POINT_MIP_LINEAR: Filter = Filter(1);
/// The header's `D3D12_FILTER_MIN_POINT_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_POINT_MAG_LINEAR_MIP_POINT: Filter = Filter(4);
/// The header's `D3D12_FILTER_MIN_POINT_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_POINT_MAG_MIP_LINEAR: Filter = Filter(5);
/// The header's `D3D12_FILTER_MIN_LINEAR_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_LINEAR_MAG_MIP_POINT: Filter = Filter(16);
/// The header's `D3D12_FILTER_MIN_LINEAR_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_LINEAR_MAG_POINT_MIP_LINEAR: Filter = Filter(17);
/// The header's `D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT: Filter = Filter(20);
/// The header's `D3D12_FILTER_MIN_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_MAG_MIP_LINEAR: Filter = Filter(21);
/// The header's `D3D12_FILTER_MIN_MAG_ANISOTROPIC_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MIN_MAG_ANISOTROPIC_MIP_POINT: Filter = Filter(84);
/// The header's `D3D12_FILTER_ANISOTROPIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_ANISOTROPIC: Filter = Filter(85);
/// The header's `D3D12_FILTER_COMPARISON_MIN_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_MAG_MIP_POINT: Filter = Filter(128);
/// The header's `D3D12_FILTER_COMPARISON_MIN_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_MAG_POINT_MIP_LINEAR: Filter = Filter(129);
/// The header's `D3D12_FILTER_COMPARISON_MIN_POINT_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_POINT_MAG_LINEAR_MIP_POINT: Filter = Filter(132);
/// The header's `D3D12_FILTER_COMPARISON_MIN_POINT_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_POINT_MAG_MIP_LINEAR: Filter = Filter(133);
/// The header's `D3D12_FILTER_COMPARISON_MIN_LINEAR_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_LINEAR_MAG_MIP_POINT: Filter = Filter(144);
/// The header's `D3D12_FILTER_COMPARISON_MIN_LINEAR_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_LINEAR_MAG_POINT_MIP_LINEAR: Filter = Filter(145);
/// The header's `D3D12_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT: Filter = Filter(148);
/// The header's `D3D12_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR: Filter = Filter(149);
/// The header's `D3D12_FILTER_COMPARISON_MIN_MAG_ANISOTROPIC_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_MIN_MAG_ANISOTROPIC_MIP_POINT: Filter = Filter(212);
/// The header's `D3D12_FILTER_COMPARISON_ANISOTROPIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_COMPARISON_ANISOTROPIC: Filter = Filter(213);
/// The header's `D3D12_FILTER_MINIMUM_MIN_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_MAG_MIP_POINT: Filter = Filter(0x100);
/// The header's `D3D12_FILTER_MINIMUM_MIN_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_MAG_POINT_MIP_LINEAR: Filter = Filter(0x101);
/// The header's `D3D12_FILTER_MINIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT: Filter = Filter(0x104);
/// The header's `D3D12_FILTER_MINIMUM_MIN_POINT_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_POINT_MAG_MIP_LINEAR: Filter = Filter(0x105);
/// The header's `D3D12_FILTER_MINIMUM_MIN_LINEAR_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_LINEAR_MAG_MIP_POINT: Filter = Filter(0x110);
/// The header's `D3D12_FILTER_MINIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR: Filter = Filter(0x111);
/// The header's `D3D12_FILTER_MINIMUM_MIN_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_MAG_LINEAR_MIP_POINT: Filter = Filter(0x114);
/// The header's `D3D12_FILTER_MINIMUM_MIN_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_MAG_MIP_LINEAR: Filter = Filter(0x115);
/// The header's `D3D12_FILTER_MINIMUM_MIN_MAG_ANISOTROPIC_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_MIN_MAG_ANISOTROPIC_MIP_POINT: Filter = Filter(0x154);
/// The header's `D3D12_FILTER_MINIMUM_ANISOTROPIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MINIMUM_ANISOTROPIC: Filter = Filter(0x155);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_MAG_MIP_POINT: Filter = Filter(0x180);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_MAG_POINT_MIP_LINEAR: Filter = Filter(0x181);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT: Filter = Filter(0x184);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_POINT_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_POINT_MAG_MIP_LINEAR: Filter = Filter(0x185);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_LINEAR_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_LINEAR_MAG_MIP_POINT: Filter = Filter(0x190);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR: Filter = Filter(0x191);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_MAG_LINEAR_MIP_POINT: Filter = Filter(0x194);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_MAG_MIP_LINEAR: Filter = Filter(0x195);
/// The header's `D3D12_FILTER_MAXIMUM_MIN_MAG_ANISOTROPIC_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_MIN_MAG_ANISOTROPIC_MIP_POINT: Filter = Filter(0x1d4);
/// The header's `D3D12_FILTER_MAXIMUM_ANISOTROPIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter).
pub const D3D12_FILTER_MAXIMUM_ANISOTROPIC: Filter = Filter(0x1d5);

/// The header's `D3D12_FILTER_REDUCTION_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter_reduction_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FilterReductionType(pub i32);

/// The header's `D3D12_FILTER_REDUCTION_TYPE_STANDARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter_reduction_type).
pub const D3D12_FILTER_REDUCTION_TYPE_STANDARD: FilterReductionType = FilterReductionType(0);
/// The header's `D3D12_FILTER_REDUCTION_TYPE_COMPARISON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter_reduction_type).
pub const D3D12_FILTER_REDUCTION_TYPE_COMPARISON: FilterReductionType = FilterReductionType(1);
/// The header's `D3D12_FILTER_REDUCTION_TYPE_MINIMUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter_reduction_type).
pub const D3D12_FILTER_REDUCTION_TYPE_MINIMUM: FilterReductionType = FilterReductionType(2);
/// The header's `D3D12_FILTER_REDUCTION_TYPE_MAXIMUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter_reduction_type).
pub const D3D12_FILTER_REDUCTION_TYPE_MAXIMUM: FilterReductionType = FilterReductionType(3);

/// The header's `D3D12_FILTER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FilterType(pub i32);

/// The header's `D3D12_FILTER_TYPE_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter_type).
pub const D3D12_FILTER_TYPE_POINT: FilterType = FilterType(0);
/// The header's `D3D12_FILTER_TYPE_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_filter_type).
pub const D3D12_FILTER_TYPE_LINEAR: FilterType = FilterType(1);

/// The header's `D3D12_FORMAT_SUPPORT1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatSupport1(pub i32);

/// The header's `D3D12_FORMAT_SUPPORT1_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_NONE: FormatSupport1 = FormatSupport1(0);
/// The header's `D3D12_FORMAT_SUPPORT1_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_BUFFER: FormatSupport1 = FormatSupport1(1);
/// The header's `D3D12_FORMAT_SUPPORT1_IA_VERTEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_IA_VERTEX_BUFFER: FormatSupport1 = FormatSupport1(2);
/// The header's `D3D12_FORMAT_SUPPORT1_IA_INDEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_IA_INDEX_BUFFER: FormatSupport1 = FormatSupport1(4);
/// The header's `D3D12_FORMAT_SUPPORT1_SO_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_SO_BUFFER: FormatSupport1 = FormatSupport1(8);
/// The header's `D3D12_FORMAT_SUPPORT1_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_TEXTURE1D: FormatSupport1 = FormatSupport1(16);
/// The header's `D3D12_FORMAT_SUPPORT1_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_TEXTURE2D: FormatSupport1 = FormatSupport1(32);
/// The header's `D3D12_FORMAT_SUPPORT1_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_TEXTURE3D: FormatSupport1 = FormatSupport1(64);
/// The header's `D3D12_FORMAT_SUPPORT1_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_TEXTURECUBE: FormatSupport1 = FormatSupport1(128);
/// The header's `D3D12_FORMAT_SUPPORT1_SHADER_LOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_SHADER_LOAD: FormatSupport1 = FormatSupport1(0x100);
/// The header's `D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE: FormatSupport1 = FormatSupport1(0x200);
/// The header's `D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE_COMPARISON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE_COMPARISON: FormatSupport1 = FormatSupport1(0x400);
/// The header's `D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE_MONO_TEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE_MONO_TEXT: FormatSupport1 = FormatSupport1(0x800);
/// The header's `D3D12_FORMAT_SUPPORT1_MIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_MIP: FormatSupport1 = FormatSupport1(0x1000);
/// The header's `D3D12_FORMAT_SUPPORT1_RENDER_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_RENDER_TARGET: FormatSupport1 = FormatSupport1(0x4000);
/// The header's `D3D12_FORMAT_SUPPORT1_BLENDABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_BLENDABLE: FormatSupport1 = FormatSupport1(0x8000);
/// The header's `D3D12_FORMAT_SUPPORT1_DEPTH_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_DEPTH_STENCIL: FormatSupport1 = FormatSupport1(0x10000);
/// The header's `D3D12_FORMAT_SUPPORT1_MULTISAMPLE_RESOLVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_MULTISAMPLE_RESOLVE: FormatSupport1 = FormatSupport1(0x40000);
/// The header's `D3D12_FORMAT_SUPPORT1_DISPLAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_DISPLAY: FormatSupport1 = FormatSupport1(0x80000);
/// The header's `D3D12_FORMAT_SUPPORT1_CAST_WITHIN_BIT_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_CAST_WITHIN_BIT_LAYOUT: FormatSupport1 = FormatSupport1(0x100000);
/// The header's `D3D12_FORMAT_SUPPORT1_MULTISAMPLE_RENDERTARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_MULTISAMPLE_RENDERTARGET: FormatSupport1 = FormatSupport1(0x200000);
/// The header's `D3D12_FORMAT_SUPPORT1_MULTISAMPLE_LOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_MULTISAMPLE_LOAD: FormatSupport1 = FormatSupport1(0x400000);
/// The header's `D3D12_FORMAT_SUPPORT1_SHADER_GATHER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_SHADER_GATHER: FormatSupport1 = FormatSupport1(0x800000);
/// The header's `D3D12_FORMAT_SUPPORT1_BACK_BUFFER_CAST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_BACK_BUFFER_CAST: FormatSupport1 = FormatSupport1(0x1000000);
/// The header's `D3D12_FORMAT_SUPPORT1_TYPED_UNORDERED_ACCESS_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_TYPED_UNORDERED_ACCESS_VIEW: FormatSupport1 =
    FormatSupport1(0x2000000);
/// The header's `D3D12_FORMAT_SUPPORT1_SHADER_GATHER_COMPARISON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_SHADER_GATHER_COMPARISON: FormatSupport1 =
    FormatSupport1(0x4000000);
/// The header's `D3D12_FORMAT_SUPPORT1_DECODER_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_DECODER_OUTPUT: FormatSupport1 = FormatSupport1(0x8000000);
/// The header's `D3D12_FORMAT_SUPPORT1_VIDEO_PROCESSOR_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_VIDEO_PROCESSOR_OUTPUT: FormatSupport1 = FormatSupport1(0x10000000);
/// The header's `D3D12_FORMAT_SUPPORT1_VIDEO_PROCESSOR_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_VIDEO_PROCESSOR_INPUT: FormatSupport1 = FormatSupport1(0x20000000);
/// The header's `D3D12_FORMAT_SUPPORT1_VIDEO_ENCODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support1).
pub const D3D12_FORMAT_SUPPORT1_VIDEO_ENCODER: FormatSupport1 = FormatSupport1(0x40000000);

/// The header's `D3D12_FORMAT_SUPPORT2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatSupport2(pub i32);

/// The header's `D3D12_FORMAT_SUPPORT2_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_NONE: FormatSupport2 = FormatSupport2(0);
/// The header's `D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_ADD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_ADD: FormatSupport2 = FormatSupport2(1);
/// The header's `D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_BITWISE_OPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_BITWISE_OPS: FormatSupport2 = FormatSupport2(2);
/// The header's `D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE: FormatSupport2 =
    FormatSupport2(4);
/// The header's `D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_EXCHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_EXCHANGE: FormatSupport2 = FormatSupport2(8);
/// The header's `D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_SIGNED_MIN_OR_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_SIGNED_MIN_OR_MAX: FormatSupport2 = FormatSupport2(16);
/// The header's `D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_UNSIGNED_MIN_OR_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_UAV_ATOMIC_UNSIGNED_MIN_OR_MAX: FormatSupport2 = FormatSupport2(32);
/// The header's `D3D12_FORMAT_SUPPORT2_UAV_TYPED_LOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_UAV_TYPED_LOAD: FormatSupport2 = FormatSupport2(64);
/// The header's `D3D12_FORMAT_SUPPORT2_UAV_TYPED_STORE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_UAV_TYPED_STORE: FormatSupport2 = FormatSupport2(128);
/// The header's `D3D12_FORMAT_SUPPORT2_OUTPUT_MERGER_LOGIC_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_OUTPUT_MERGER_LOGIC_OP: FormatSupport2 = FormatSupport2(0x100);
/// The header's `D3D12_FORMAT_SUPPORT2_TILED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_TILED: FormatSupport2 = FormatSupport2(0x200);
/// The header's `D3D12_FORMAT_SUPPORT2_MULTIPLANE_OVERLAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_MULTIPLANE_OVERLAY: FormatSupport2 = FormatSupport2(0x4000);
/// The header's `D3D12_FORMAT_SUPPORT2_SAMPLER_FEEDBACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_format_support2).
pub const D3D12_FORMAT_SUPPORT2_SAMPLER_FEEDBACK: FormatSupport2 = FormatSupport2(0x8000);

/// The header's `D3D12_GPU_BASED_VALIDATION_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GpuBasedValidationFlags(pub i32);

/// The header's `D3D12_GPU_BASED_VALIDATION_FLAGS_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_flags).
pub const D3D12_GPU_BASED_VALIDATION_FLAGS_NONE: GpuBasedValidationFlags =
    GpuBasedValidationFlags(0);
/// The header's `D3D12_GPU_BASED_VALIDATION_FLAGS_DISABLE_STATE_TRACKING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_flags).
pub const D3D12_GPU_BASED_VALIDATION_FLAGS_DISABLE_STATE_TRACKING: GpuBasedValidationFlags =
    GpuBasedValidationFlags(1);

/// The header's `D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_pipeline_state_create_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GpuBasedValidationPipelineStateCreateFlags(pub i32);

/// The header's `D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_pipeline_state_create_flags).
pub const D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAG_NONE:
    GpuBasedValidationPipelineStateCreateFlags = GpuBasedValidationPipelineStateCreateFlags(0);
/// The header's `D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAG_FRONT_LOAD_CREATE_TRACKING_ONLY_SHADERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_pipeline_state_create_flags).
pub const D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAG_FRONT_LOAD_CREATE_TRACKING_ONLY_SHADERS: GpuBasedValidationPipelineStateCreateFlags = GpuBasedValidationPipelineStateCreateFlags(1);
/// The header's `D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAG_FRONT_LOAD_CREATE_UNGUARDED_VALIDATION_SHADERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_pipeline_state_create_flags).
pub const D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAG_FRONT_LOAD_CREATE_UNGUARDED_VALIDATION_SHADERS: GpuBasedValidationPipelineStateCreateFlags = GpuBasedValidationPipelineStateCreateFlags(2);
/// The header's `D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAG_FRONT_LOAD_CREATE_GUARDED_VALIDATION_SHADERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_pipeline_state_create_flags).
pub const D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAG_FRONT_LOAD_CREATE_GUARDED_VALIDATION_SHADERS: GpuBasedValidationPipelineStateCreateFlags = GpuBasedValidationPipelineStateCreateFlags(4);
/// The header's `D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAGS_VALID_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_pipeline_state_create_flags).
pub const D3D12_GPU_BASED_VALIDATION_PIPELINE_STATE_CREATE_FLAGS_VALID_MASK:
    GpuBasedValidationPipelineStateCreateFlags = GpuBasedValidationPipelineStateCreateFlags(7);

/// The header's `D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_shader_patch_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GpuBasedValidationShaderPatchMode(pub i32);

/// The header's `D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_shader_patch_mode).
pub const D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE_NONE: GpuBasedValidationShaderPatchMode =
    GpuBasedValidationShaderPatchMode(0);
/// The header's `D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE_STATE_TRACKING_ONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_shader_patch_mode).
pub const D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE_STATE_TRACKING_ONLY:
    GpuBasedValidationShaderPatchMode = GpuBasedValidationShaderPatchMode(1);
/// The header's `D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE_UNGUARDED_VALIDATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_shader_patch_mode).
pub const D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE_UNGUARDED_VALIDATION:
    GpuBasedValidationShaderPatchMode = GpuBasedValidationShaderPatchMode(2);
/// The header's `D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE_GUARDED_VALIDATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_shader_patch_mode).
pub const D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODE_GUARDED_VALIDATION:
    GpuBasedValidationShaderPatchMode = GpuBasedValidationShaderPatchMode(3);
/// The header's `NUM_D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_gpu_based_validation_shader_patch_mode).
pub const NUM_D3D12_GPU_BASED_VALIDATION_SHADER_PATCH_MODES: GpuBasedValidationShaderPatchMode =
    GpuBasedValidationShaderPatchMode(4);

/// The header's `D3D12_GRAPHICS_STATES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GraphicsStates(pub i32);

/// The header's `D3D12_GRAPHICS_STATE_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_NONE: GraphicsStates = GraphicsStates(0);
/// The header's `D3D12_GRAPHICS_STATE_IA_VERTEX_BUFFERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_IA_VERTEX_BUFFERS: GraphicsStates = GraphicsStates(1);
/// The header's `D3D12_GRAPHICS_STATE_IA_INDEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_IA_INDEX_BUFFER: GraphicsStates = GraphicsStates(2);
/// The header's `D3D12_GRAPHICS_STATE_IA_PRIMITIVE_TOPOLOGY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_IA_PRIMITIVE_TOPOLOGY: GraphicsStates = GraphicsStates(4);
/// The header's `D3D12_GRAPHICS_STATE_DESCRIPTOR_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_DESCRIPTOR_HEAP: GraphicsStates = GraphicsStates(8);
/// The header's `D3D12_GRAPHICS_STATE_GRAPHICS_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_GRAPHICS_ROOT_SIGNATURE: GraphicsStates = GraphicsStates(16);
/// The header's `D3D12_GRAPHICS_STATE_COMPUTE_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_COMPUTE_ROOT_SIGNATURE: GraphicsStates = GraphicsStates(32);
/// The header's `D3D12_GRAPHICS_STATE_RS_VIEWPORTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_RS_VIEWPORTS: GraphicsStates = GraphicsStates(64);
/// The header's `D3D12_GRAPHICS_STATE_RS_SCISSOR_RECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_RS_SCISSOR_RECTS: GraphicsStates = GraphicsStates(128);
/// The header's `D3D12_GRAPHICS_STATE_PREDICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_PREDICATION: GraphicsStates = GraphicsStates(0x100);
/// The header's `D3D12_GRAPHICS_STATE_OM_RENDER_TARGETS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_OM_RENDER_TARGETS: GraphicsStates = GraphicsStates(0x200);
/// The header's `D3D12_GRAPHICS_STATE_OM_STENCIL_REF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_OM_STENCIL_REF: GraphicsStates = GraphicsStates(0x400);
/// The header's `D3D12_GRAPHICS_STATE_OM_BLEND_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_OM_BLEND_FACTOR: GraphicsStates = GraphicsStates(0x800);
/// The header's `D3D12_GRAPHICS_STATE_PIPELINE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_PIPELINE_STATE: GraphicsStates = GraphicsStates(0x1000);
/// The header's `D3D12_GRAPHICS_STATE_SO_TARGETS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_SO_TARGETS: GraphicsStates = GraphicsStates(0x2000);
/// The header's `D3D12_GRAPHICS_STATE_OM_DEPTH_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_OM_DEPTH_BOUNDS: GraphicsStates = GraphicsStates(0x4000);
/// The header's `D3D12_GRAPHICS_STATE_SAMPLE_POSITIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_SAMPLE_POSITIONS: GraphicsStates = GraphicsStates(0x8000);
/// The header's `D3D12_GRAPHICS_STATE_VIEW_INSTANCE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_graphics_states).
pub const D3D12_GRAPHICS_STATE_VIEW_INSTANCE_MASK: GraphicsStates = GraphicsStates(0x10000);

/// The header's `D3D12_HEAP_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HeapFlags(pub i32);

/// The header's `D3D12_HEAP_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_NONE: HeapFlags = HeapFlags(0);
/// The header's `D3D12_HEAP_FLAG_SHARED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_SHARED: HeapFlags = HeapFlags(1);
/// The header's `D3D12_HEAP_FLAG_DENY_BUFFERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_DENY_BUFFERS: HeapFlags = HeapFlags(4);
/// The header's `D3D12_HEAP_FLAG_ALLOW_DISPLAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_ALLOW_DISPLAY: HeapFlags = HeapFlags(8);
/// The header's `D3D12_HEAP_FLAG_SHARED_CROSS_ADAPTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_SHARED_CROSS_ADAPTER: HeapFlags = HeapFlags(32);
/// The header's `D3D12_HEAP_FLAG_DENY_RT_DS_TEXTURES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_DENY_RT_DS_TEXTURES: HeapFlags = HeapFlags(64);
/// The header's `D3D12_HEAP_FLAG_DENY_NON_RT_DS_TEXTURES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_DENY_NON_RT_DS_TEXTURES: HeapFlags = HeapFlags(128);
/// The header's `D3D12_HEAP_FLAG_HARDWARE_PROTECTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_HARDWARE_PROTECTED: HeapFlags = HeapFlags(0x100);
/// The header's `D3D12_HEAP_FLAG_ALLOW_WRITE_WATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_ALLOW_WRITE_WATCH: HeapFlags = HeapFlags(0x200);
/// The header's `D3D12_HEAP_FLAG_ALLOW_SHADER_ATOMICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_ALLOW_SHADER_ATOMICS: HeapFlags = HeapFlags(0x400);
/// The header's `D3D12_HEAP_FLAG_CREATE_NOT_RESIDENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_CREATE_NOT_RESIDENT: HeapFlags = HeapFlags(0x800);
/// The header's `D3D12_HEAP_FLAG_CREATE_NOT_ZEROED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_CREATE_NOT_ZEROED: HeapFlags = HeapFlags(0x1000);
/// The header's `D3D12_HEAP_FLAG_TOOLS_USE_MANUAL_WRITE_TRACKING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_TOOLS_USE_MANUAL_WRITE_TRACKING: HeapFlags = HeapFlags(0x2000);
/// The header's `D3D12_HEAP_FLAG_ALLOW_ALL_BUFFERS_AND_TEXTURES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_ALLOW_ALL_BUFFERS_AND_TEXTURES: HeapFlags = HeapFlags(0);
/// The header's `D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS: HeapFlags = HeapFlags(192);
/// The header's `D3D12_HEAP_FLAG_ALLOW_ONLY_NON_RT_DS_TEXTURES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_ALLOW_ONLY_NON_RT_DS_TEXTURES: HeapFlags = HeapFlags(68);
/// The header's `D3D12_HEAP_FLAG_ALLOW_ONLY_RT_DS_TEXTURES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_flags).
pub const D3D12_HEAP_FLAG_ALLOW_ONLY_RT_DS_TEXTURES: HeapFlags = HeapFlags(132);

/// The header's `D3D12_HEAP_SERIALIZATION_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_serialization_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HeapSerializationTier(pub i32);

/// The header's `D3D12_HEAP_SERIALIZATION_TIER_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_serialization_tier).
pub const D3D12_HEAP_SERIALIZATION_TIER_0: HeapSerializationTier = HeapSerializationTier(0);
/// The header's `D3D12_HEAP_SERIALIZATION_TIER_10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_serialization_tier).
pub const D3D12_HEAP_SERIALIZATION_TIER_10: HeapSerializationTier = HeapSerializationTier(10);

/// The header's `D3D12_HEAP_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HeapType(pub i32);

/// The header's `D3D12_HEAP_TYPE_DEFAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_type).
pub const D3D12_HEAP_TYPE_DEFAULT: HeapType = HeapType(1);
/// The header's `D3D12_HEAP_TYPE_UPLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_type).
pub const D3D12_HEAP_TYPE_UPLOAD: HeapType = HeapType(2);
/// The header's `D3D12_HEAP_TYPE_READBACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_type).
pub const D3D12_HEAP_TYPE_READBACK: HeapType = HeapType(3);
/// The header's `D3D12_HEAP_TYPE_CUSTOM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_type).
pub const D3D12_HEAP_TYPE_CUSTOM: HeapType = HeapType(4);
/// The header's `D3D12_HEAP_TYPE_GPU_UPLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_type).
pub const D3D12_HEAP_TYPE_GPU_UPLOAD: HeapType = HeapType(5);

/// The header's `D3D12_HIT_GROUP_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_hit_group_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HitGroupType(pub i32);

/// The header's `D3D12_HIT_GROUP_TYPE_TRIANGLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_hit_group_type).
pub const D3D12_HIT_GROUP_TYPE_TRIANGLES: HitGroupType = HitGroupType(0);
/// The header's `D3D12_HIT_GROUP_TYPE_PROCEDURAL_PRIMITIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_hit_group_type).
pub const D3D12_HIT_GROUP_TYPE_PROCEDURAL_PRIMITIVE: HitGroupType = HitGroupType(1);

/// The header's `D3D12_HIT_KIND`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HitKind(pub i32);

/// The header's `D3D12_HIT_KIND_TRIANGLE_FRONT_FACE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_HIT_KIND_TRIANGLE_FRONT_FACE: HitKind = HitKind(254);
/// The header's `D3D12_HIT_KIND_TRIANGLE_BACK_FACE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_HIT_KIND_TRIANGLE_BACK_FACE: HitKind = HitKind(255);

/// The header's `D3D12_INDEX_BUFFER_STRIP_CUT_VALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_index_buffer_strip_cut_value).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct IndexBufferStripCutValue(pub i32);

/// The header's `D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_index_buffer_strip_cut_value).
pub const D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED: IndexBufferStripCutValue =
    IndexBufferStripCutValue(0);
/// The header's `D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0xFFFF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_index_buffer_strip_cut_value).
pub const D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0XFFFF: IndexBufferStripCutValue =
    IndexBufferStripCutValue(1);
/// The header's `D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0xFFFFFFFF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_index_buffer_strip_cut_value).
pub const D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0XFFFFFFFF: IndexBufferStripCutValue =
    IndexBufferStripCutValue(2);

/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct IndirectArgumentType(pub i32);

/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_DRAW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_DRAW: IndirectArgumentType = IndirectArgumentType(0);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED: IndirectArgumentType = IndirectArgumentType(1);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH: IndirectArgumentType = IndirectArgumentType(2);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_VERTEX_BUFFER_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_VERTEX_BUFFER_VIEW: IndirectArgumentType =
    IndirectArgumentType(3);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_INDEX_BUFFER_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_INDEX_BUFFER_VIEW: IndirectArgumentType =
    IndirectArgumentType(4);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT: IndirectArgumentType = IndirectArgumentType(5);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT_BUFFER_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT_BUFFER_VIEW: IndirectArgumentType =
    IndirectArgumentType(6);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_SHADER_RESOURCE_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_SHADER_RESOURCE_VIEW: IndirectArgumentType =
    IndirectArgumentType(7);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_UNORDERED_ACCESS_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_UNORDERED_ACCESS_VIEW: IndirectArgumentType =
    IndirectArgumentType(8);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH_RAYS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH_RAYS: IndirectArgumentType =
    IndirectArgumentType(9);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH_MESH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH_MESH: IndirectArgumentType =
    IndirectArgumentType(10);
/// The header's `D3D12_INDIRECT_ARGUMENT_TYPE_INCREMENTING_CONSTANT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_indirect_argument_type).
pub const D3D12_INDIRECT_ARGUMENT_TYPE_INCREMENTING_CONSTANT: IndirectArgumentType =
    IndirectArgumentType(11);

/// The header's `D3D12_INPUT_CLASSIFICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_input_classification).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InputClassification(pub i32);

/// The header's `D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_input_classification).
pub const D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA: InputClassification = InputClassification(0);
/// The header's `D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_input_classification).
pub const D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA: InputClassification =
    InputClassification(1);

/// The header's `D3D12_LIFETIME_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_lifetime_state).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LifetimeState(pub i32);

/// The header's `D3D12_LIFETIME_STATE_IN_USE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_lifetime_state).
pub const D3D12_LIFETIME_STATE_IN_USE: LifetimeState = LifetimeState(0);
/// The header's `D3D12_LIFETIME_STATE_NOT_IN_USE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_lifetime_state).
pub const D3D12_LIFETIME_STATE_NOT_IN_USE: LifetimeState = LifetimeState(1);

/// The header's `D3D12_LINE_RASTERIZATION_MODE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LineRasterizationMode(pub i32);

/// The header's `D3D12_LINE_RASTERIZATION_MODE_ALIASED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_LINE_RASTERIZATION_MODE_ALIASED: LineRasterizationMode = LineRasterizationMode(0);
/// The header's `D3D12_LINE_RASTERIZATION_MODE_ALPHA_ANTIALIASED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_LINE_RASTERIZATION_MODE_ALPHA_ANTIALIASED: LineRasterizationMode =
    LineRasterizationMode(1);
/// The header's `D3D12_LINE_RASTERIZATION_MODE_QUADRILATERAL_WIDE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_LINE_RASTERIZATION_MODE_QUADRILATERAL_WIDE: LineRasterizationMode =
    LineRasterizationMode(2);
/// The header's `D3D12_LINE_RASTERIZATION_MODE_QUADRILATERAL_NARROW`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_LINE_RASTERIZATION_MODE_QUADRILATERAL_NARROW: LineRasterizationMode =
    LineRasterizationMode(3);

/// The header's `D3D12_LOGIC_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LogicOp(pub i32);

/// The header's `D3D12_LOGIC_OP_CLEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_CLEAR: LogicOp = LogicOp(0);
/// The header's `D3D12_LOGIC_OP_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_SET: LogicOp = LogicOp(1);
/// The header's `D3D12_LOGIC_OP_COPY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_COPY: LogicOp = LogicOp(2);
/// The header's `D3D12_LOGIC_OP_COPY_INVERTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_COPY_INVERTED: LogicOp = LogicOp(3);
/// The header's `D3D12_LOGIC_OP_NOOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_NOOP: LogicOp = LogicOp(4);
/// The header's `D3D12_LOGIC_OP_INVERT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_INVERT: LogicOp = LogicOp(5);
/// The header's `D3D12_LOGIC_OP_AND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_AND: LogicOp = LogicOp(6);
/// The header's `D3D12_LOGIC_OP_NAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_NAND: LogicOp = LogicOp(7);
/// The header's `D3D12_LOGIC_OP_OR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_OR: LogicOp = LogicOp(8);
/// The header's `D3D12_LOGIC_OP_NOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_NOR: LogicOp = LogicOp(9);
/// The header's `D3D12_LOGIC_OP_XOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_XOR: LogicOp = LogicOp(10);
/// The header's `D3D12_LOGIC_OP_EQUIV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_EQUIV: LogicOp = LogicOp(11);
/// The header's `D3D12_LOGIC_OP_AND_REVERSE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_AND_REVERSE: LogicOp = LogicOp(12);
/// The header's `D3D12_LOGIC_OP_AND_INVERTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_AND_INVERTED: LogicOp = LogicOp(13);
/// The header's `D3D12_LOGIC_OP_OR_REVERSE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_OR_REVERSE: LogicOp = LogicOp(14);
/// The header's `D3D12_LOGIC_OP_OR_INVERTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_logic_op).
pub const D3D12_LOGIC_OP_OR_INVERTED: LogicOp = LogicOp(15);

/// The header's `D3D12_MEASUREMENTS_ACTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_measurements_action).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MeasurementsAction(pub i32);

/// The header's `D3D12_MEASUREMENTS_ACTION_KEEP_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_measurements_action).
pub const D3D12_MEASUREMENTS_ACTION_KEEP_ALL: MeasurementsAction = MeasurementsAction(0);
/// The header's `D3D12_MEASUREMENTS_ACTION_COMMIT_RESULTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_measurements_action).
pub const D3D12_MEASUREMENTS_ACTION_COMMIT_RESULTS: MeasurementsAction = MeasurementsAction(1);
/// The header's `D3D12_MEASUREMENTS_ACTION_COMMIT_RESULTS_HIGH_PRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_measurements_action).
pub const D3D12_MEASUREMENTS_ACTION_COMMIT_RESULTS_HIGH_PRIORITY: MeasurementsAction =
    MeasurementsAction(2);
/// The header's `D3D12_MEASUREMENTS_ACTION_DISCARD_PREVIOUS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_measurements_action).
pub const D3D12_MEASUREMENTS_ACTION_DISCARD_PREVIOUS: MeasurementsAction = MeasurementsAction(3);

/// The header's `D3D12_MEMORY_POOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_memory_pool).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MemoryPool(pub i32);

/// The header's `D3D12_MEMORY_POOL_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_memory_pool).
pub const D3D12_MEMORY_POOL_UNKNOWN: MemoryPool = MemoryPool(0);
/// The header's `D3D12_MEMORY_POOL_L0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_memory_pool).
pub const D3D12_MEMORY_POOL_L0: MemoryPool = MemoryPool(1);
/// The header's `D3D12_MEMORY_POOL_L1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_memory_pool).
pub const D3D12_MEMORY_POOL_L1: MemoryPool = MemoryPool(2);

/// The header's `D3D12_MESH_SHADER_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_mesh_shader_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MeshShaderTier(pub i32);

/// The header's `D3D12_MESH_SHADER_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_mesh_shader_tier).
pub const D3D12_MESH_SHADER_TIER_NOT_SUPPORTED: MeshShaderTier = MeshShaderTier(0);
/// The header's `D3D12_MESH_SHADER_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_mesh_shader_tier).
pub const D3D12_MESH_SHADER_TIER_1: MeshShaderTier = MeshShaderTier(10);

/// The header's `D3D12_MESSAGE_CALLBACK_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MessageCallbackFlags(pub i32);

/// The header's `D3D12_MESSAGE_CALLBACK_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_MESSAGE_CALLBACK_FLAG_NONE: MessageCallbackFlags = MessageCallbackFlags(0);
/// The header's `D3D12_MESSAGE_CALLBACK_IGNORE_FILTERS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_MESSAGE_CALLBACK_IGNORE_FILTERS: MessageCallbackFlags = MessageCallbackFlags(1);

/// The header's `D3D12_MESSAGE_CATEGORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MessageCategory(pub i32);

/// The header's `D3D12_MESSAGE_CATEGORY_APPLICATION_DEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_APPLICATION_DEFINED: MessageCategory = MessageCategory(0);
/// The header's `D3D12_MESSAGE_CATEGORY_MISCELLANEOUS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_MISCELLANEOUS: MessageCategory = MessageCategory(1);
/// The header's `D3D12_MESSAGE_CATEGORY_INITIALIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_INITIALIZATION: MessageCategory = MessageCategory(2);
/// The header's `D3D12_MESSAGE_CATEGORY_CLEANUP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_CLEANUP: MessageCategory = MessageCategory(3);
/// The header's `D3D12_MESSAGE_CATEGORY_COMPILATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_COMPILATION: MessageCategory = MessageCategory(4);
/// The header's `D3D12_MESSAGE_CATEGORY_STATE_CREATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_STATE_CREATION: MessageCategory = MessageCategory(5);
/// The header's `D3D12_MESSAGE_CATEGORY_STATE_SETTING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_STATE_SETTING: MessageCategory = MessageCategory(6);
/// The header's `D3D12_MESSAGE_CATEGORY_STATE_GETTING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_STATE_GETTING: MessageCategory = MessageCategory(7);
/// The header's `D3D12_MESSAGE_CATEGORY_RESOURCE_MANIPULATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_RESOURCE_MANIPULATION: MessageCategory = MessageCategory(8);
/// The header's `D3D12_MESSAGE_CATEGORY_EXECUTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_EXECUTION: MessageCategory = MessageCategory(9);
/// The header's `D3D12_MESSAGE_CATEGORY_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_category).
pub const D3D12_MESSAGE_CATEGORY_SHADER: MessageCategory = MessageCategory(10);

/// The header's `D3D12_MESSAGE_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MessageId(pub i32);

/// The header's `D3D12_MESSAGE_ID_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNKNOWN: MessageId = MessageId(0);
/// The header's `D3D12_MESSAGE_ID_STRING_FROM_APPLICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_STRING_FROM_APPLICATION: MessageId = MessageId(1);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_THIS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_THIS: MessageId = MessageId(2);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER1: MessageId = MessageId(3);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER2: MessageId = MessageId(4);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER3: MessageId = MessageId(5);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER4: MessageId = MessageId(6);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER5: MessageId = MessageId(7);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER6: MessageId = MessageId(8);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER7: MessageId = MessageId(9);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER8: MessageId = MessageId(10);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER9`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER9: MessageId = MessageId(11);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER10: MessageId = MessageId(12);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER11`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER11: MessageId = MessageId(13);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER12`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER12: MessageId = MessageId(14);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER13`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER13: MessageId = MessageId(15);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER14`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER14: MessageId = MessageId(16);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_PARAMETER15`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_PARAMETER15: MessageId = MessageId(17);
/// The header's `D3D12_MESSAGE_ID_CORRUPTED_MULTITHREADING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CORRUPTED_MULTITHREADING: MessageId = MessageId(18);
/// The header's `D3D12_MESSAGE_ID_MESSAGE_REPORTING_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MESSAGE_REPORTING_OUTOFMEMORY: MessageId = MessageId(19);
/// The header's `D3D12_MESSAGE_ID_GETPRIVATEDATA_MOREDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETPRIVATEDATA_MOREDATA: MessageId = MessageId(20);
/// The header's `D3D12_MESSAGE_ID_SETPRIVATEDATA_INVALIDFREEDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETPRIVATEDATA_INVALIDFREEDATA: MessageId = MessageId(21);
/// The header's `D3D12_MESSAGE_ID_SETPRIVATEDATA_CHANGINGPARAMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETPRIVATEDATA_CHANGINGPARAMS: MessageId = MessageId(24);
/// The header's `D3D12_MESSAGE_ID_SETPRIVATEDATA_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETPRIVATEDATA_OUTOFMEMORY: MessageId = MessageId(25);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_UNRECOGNIZEDFORMAT: MessageId = MessageId(26);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDESC: MessageId = MessageId(27);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDFORMAT: MessageId = MessageId(28);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDVIDEOPLANESLICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDVIDEOPLANESLICE: MessageId =
    MessageId(29);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDPLANESLICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDPLANESLICE: MessageId = MessageId(30);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDIMENSIONS: MessageId = MessageId(31);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDRESOURCE: MessageId = MessageId(32);
/// The header's `D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_UNRECOGNIZEDFORMAT: MessageId = MessageId(35);
/// The header's `D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_UNSUPPORTEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_UNSUPPORTEDFORMAT: MessageId = MessageId(36);
/// The header's `D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDESC: MessageId = MessageId(37);
/// The header's `D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDFORMAT: MessageId = MessageId(38);
/// The header's `D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDVIDEOPLANESLICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDVIDEOPLANESLICE: MessageId = MessageId(39);
/// The header's `D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDPLANESLICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDPLANESLICE: MessageId = MessageId(40);
/// The header's `D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDIMENSIONS: MessageId = MessageId(41);
/// The header's `D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDRESOURCE: MessageId = MessageId(42);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_UNRECOGNIZEDFORMAT: MessageId = MessageId(45);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDDESC: MessageId = MessageId(46);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDFORMAT: MessageId = MessageId(47);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDDIMENSIONS: MessageId = MessageId(48);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDRESOURCE: MessageId = MessageId(49);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_OUTOFMEMORY: MessageId = MessageId(52);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_TOOMANYELEMENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_TOOMANYELEMENTS: MessageId = MessageId(53);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDFORMAT: MessageId = MessageId(54);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INCOMPATIBLEFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INCOMPATIBLEFORMAT: MessageId = MessageId(55);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSLOT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSLOT: MessageId = MessageId(56);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDINPUTSLOTCLASS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDINPUTSLOTCLASS: MessageId = MessageId(57);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_STEPRATESLOTCLASSMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_STEPRATESLOTCLASSMISMATCH: MessageId = MessageId(58);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSLOTCLASSCHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSLOTCLASSCHANGE: MessageId = MessageId(59);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSTEPRATECHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSTEPRATECHANGE: MessageId = MessageId(60);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDALIGNMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDALIGNMENT: MessageId = MessageId(61);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_DUPLICATESEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_DUPLICATESEMANTIC: MessageId = MessageId(62);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_UNPARSEABLEINPUTSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_UNPARSEABLEINPUTSIGNATURE: MessageId = MessageId(63);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_NULLSEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_NULLSEMANTIC: MessageId = MessageId(64);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_MISSINGELEMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_MISSINGELEMENT: MessageId = MessageId(65);
/// The header's `D3D12_MESSAGE_ID_CREATEVERTEXSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEVERTEXSHADER_OUTOFMEMORY: MessageId = MessageId(66);
/// The header's `D3D12_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(67);
/// The header's `D3D12_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDSHADERTYPE: MessageId = MessageId(68);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADER_OUTOFMEMORY: MessageId = MessageId(69);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(70);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDSHADERTYPE: MessageId = MessageId(71);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTOFMEMORY: MessageId =
    MessageId(72);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSHADERBYTECODE: MessageId =
    MessageId(73);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSHADERTYPE: MessageId =
    MessageId(74);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMENTRIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMENTRIES: MessageId =
    MessageId(75);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTPUTSTREAMSTRIDEUNUSED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTPUTSTREAMSTRIDEUNUSED:
    MessageId = MessageId(76);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTPUTSLOT0EXPECTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTPUTSLOT0EXPECTED: MessageId =
    MessageId(79);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDOUTPUTSLOT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDOUTPUTSLOT: MessageId =
    MessageId(80);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_ONLYONEELEMENTPERSLOT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_ONLYONEELEMENTPERSLOT: MessageId =
    MessageId(81);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDCOMPONENTCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDCOMPONENTCOUNT: MessageId =
    MessageId(82);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTARTCOMPONENTANDCOMPONENTCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTARTCOMPONENTANDCOMPONENTCOUNT: MessageId = MessageId(83);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDGAPDEFINITION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDGAPDEFINITION: MessageId =
    MessageId(84);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_REPEATEDOUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_REPEATEDOUTPUT: MessageId =
    MessageId(85);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDOUTPUTSTREAMSTRIDE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDOUTPUTSTREAMSTRIDE:
    MessageId = MessageId(86);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MISSINGSEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MISSINGSEMANTIC: MessageId =
    MessageId(87);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MASKMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MASKMISMATCH: MessageId =
    MessageId(88);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_CANTHAVEONLYGAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_CANTHAVEONLYGAPS: MessageId =
    MessageId(89);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DECLTOOCOMPLEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DECLTOOCOMPLEX: MessageId =
    MessageId(90);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MISSINGOUTPUTSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MISSINGOUTPUTSIGNATURE: MessageId =
    MessageId(91);
/// The header's `D3D12_MESSAGE_ID_CREATEPIXELSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIXELSHADER_OUTOFMEMORY: MessageId = MessageId(92);
/// The header's `D3D12_MESSAGE_ID_CREATEPIXELSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIXELSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(93);
/// The header's `D3D12_MESSAGE_ID_CREATEPIXELSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIXELSHADER_INVALIDSHADERTYPE: MessageId = MessageId(94);
/// The header's `D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDFILLMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDFILLMODE: MessageId = MessageId(95);
/// The header's `D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDCULLMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDCULLMODE: MessageId = MessageId(96);
/// The header's `D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDDEPTHBIASCLAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDDEPTHBIASCLAMP: MessageId = MessageId(97);
/// The header's `D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDSLOPESCALEDDEPTHBIAS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDSLOPESCALEDDEPTHBIAS: MessageId =
    MessageId(98);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDDEPTHWRITEMASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDDEPTHWRITEMASK: MessageId =
    MessageId(100);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDDEPTHFUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDDEPTHFUNC: MessageId = MessageId(101);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILFAILOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILFAILOP: MessageId =
    MessageId(102);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILZFAILOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILZFAILOP: MessageId =
    MessageId(103);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILPASSOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILPASSOP: MessageId =
    MessageId(104);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILFUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILFUNC: MessageId =
    MessageId(105);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILFAILOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILFAILOP: MessageId =
    MessageId(106);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILZFAILOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILZFAILOP: MessageId =
    MessageId(107);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILPASSOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILPASSOP: MessageId =
    MessageId(108);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILFUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILFUNC: MessageId =
    MessageId(109);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDSRCBLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDSRCBLEND: MessageId = MessageId(111);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDDESTBLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDDESTBLEND: MessageId = MessageId(112);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDBLENDOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDBLENDOP: MessageId = MessageId(113);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDSRCBLENDALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDSRCBLENDALPHA: MessageId = MessageId(114);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDDESTBLENDALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDDESTBLENDALPHA: MessageId = MessageId(115);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDBLENDOPALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDBLENDOPALPHA: MessageId = MessageId(116);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDRENDERTARGETWRITEMASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDRENDERTARGETWRITEMASK: MessageId =
    MessageId(117);
/// The header's `D3D12_MESSAGE_ID_GET_PROGRAM_IDENTIFIER_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GET_PROGRAM_IDENTIFIER_ERROR: MessageId = MessageId(118);
/// The header's `D3D12_MESSAGE_ID_GET_WORK_GRAPH_PROPERTIES_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GET_WORK_GRAPH_PROPERTIES_ERROR: MessageId = MessageId(119);
/// The header's `D3D12_MESSAGE_ID_SET_PROGRAM_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_PROGRAM_ERROR: MessageId = MessageId(120);
/// The header's `D3D12_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_INVALID: MessageId = MessageId(135);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_ROOT_SIGNATURE_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_ROOT_SIGNATURE_NOT_SET: MessageId = MessageId(200);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_ROOT_SIGNATURE_MISMATCH: MessageId = MessageId(201);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_VERTEX_BUFFER_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_VERTEX_BUFFER_NOT_SET: MessageId = MessageId(202);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_VERTEX_BUFFER_STRIDE_TOO_SMALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_VERTEX_BUFFER_STRIDE_TOO_SMALL: MessageId =
    MessageId(209);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_VERTEX_BUFFER_TOO_SMALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_VERTEX_BUFFER_TOO_SMALL: MessageId = MessageId(210);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INDEX_BUFFER_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INDEX_BUFFER_NOT_SET: MessageId = MessageId(211);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INDEX_BUFFER_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INDEX_BUFFER_FORMAT_INVALID: MessageId =
    MessageId(212);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INDEX_BUFFER_TOO_SMALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INDEX_BUFFER_TOO_SMALL: MessageId = MessageId(213);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INVALID_PRIMITIVETOPOLOGY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INVALID_PRIMITIVETOPOLOGY: MessageId = MessageId(219);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_VERTEX_STRIDE_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_VERTEX_STRIDE_UNALIGNED: MessageId = MessageId(221);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INDEX_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INDEX_OFFSET_UNALIGNED: MessageId = MessageId(222);
/// The header's `D3D12_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_AT_FAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_AT_FAULT: MessageId = MessageId(232);
/// The header's `D3D12_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_POSSIBLY_AT_FAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_POSSIBLY_AT_FAULT: MessageId = MessageId(233);
/// The header's `D3D12_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_NOT_AT_FAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_NOT_AT_FAULT: MessageId = MessageId(234);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_TRAILING_DIGIT_IN_SEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_TRAILING_DIGIT_IN_SEMANTIC: MessageId = MessageId(239);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_TRAILING_DIGIT_IN_SEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_TRAILING_DIGIT_IN_SEMANTIC:
    MessageId = MessageId(240);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_TYPE_MISMATCH: MessageId = MessageId(245);
/// The header's `D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_EMPTY_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEINPUTLAYOUT_EMPTY_LAYOUT: MessageId = MessageId(253);
/// The header's `D3D12_MESSAGE_ID_LIVE_OBJECT_SUMMARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_OBJECT_SUMMARY: MessageId = MessageId(255);
/// The header's `D3D12_MESSAGE_ID_LIVE_DEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_DEVICE: MessageId = MessageId(0x112);
/// The header's `D3D12_MESSAGE_ID_LIVE_SWAPCHAIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_SWAPCHAIN: MessageId = MessageId(0x113);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDFLAGS: MessageId = MessageId(0x114);
/// The header's `D3D12_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x115);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x116);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTREAMTORASTERIZER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTREAMTORASTERIZER:
    MessageId = MessageId(0x118);
/// The header's `D3D12_MESSAGE_ID_CREATEPIXELSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIXELSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x11b);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTREAM: MessageId =
    MessageId(0x11c);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDENTRIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDENTRIES: MessageId =
    MessageId(0x11d);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDSTRIDES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDSTRIDES: MessageId =
    MessageId(0x11e);
/// The header's `D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMSTRIDES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMSTRIDES: MessageId =
    MessageId(0x11f);
/// The header's `D3D12_MESSAGE_ID_CREATEHULLSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHULLSHADER_OUTOFMEMORY: MessageId = MessageId(0x121);
/// The header's `D3D12_MESSAGE_ID_CREATEHULLSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHULLSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(0x122);
/// The header's `D3D12_MESSAGE_ID_CREATEHULLSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHULLSHADER_INVALIDSHADERTYPE: MessageId = MessageId(0x123);
/// The header's `D3D12_MESSAGE_ID_CREATEHULLSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHULLSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x124);
/// The header's `D3D12_MESSAGE_ID_CREATEDOMAINSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDOMAINSHADER_OUTOFMEMORY: MessageId = MessageId(0x126);
/// The header's `D3D12_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(0x127);
/// The header's `D3D12_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDSHADERTYPE: MessageId = MessageId(0x128);
/// The header's `D3D12_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x129);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_UNMAP_NOTMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_UNMAP_NOTMAPPED: MessageId = MessageId(0x136);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CHECKFEATURESUPPORT_MISMATCHED_DATA_SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CHECKFEATURESUPPORT_MISMATCHED_DATA_SIZE: MessageId =
    MessageId(0x13e);
/// The header's `D3D12_MESSAGE_ID_CREATECOMPUTESHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATECOMPUTESHADER_OUTOFMEMORY: MessageId = MessageId(0x141);
/// The header's `D3D12_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(0x142);
/// The header's `D3D12_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x143);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x14b);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEHULLSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEHULLSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x14c);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x14d);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x14e);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DOUBLEFLOATOPSNOTSUPPORTED: MessageId = MessageId(0x14f);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x150);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x151);
/// The header's `D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDRESOURCE: MessageId = MessageId(0x154);
/// The header's `D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDESC: MessageId = MessageId(0x155);
/// The header's `D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDFORMAT: MessageId = MessageId(0x156);
/// The header's `D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDVIDEOPLANESLICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDVIDEOPLANESLICE: MessageId =
    MessageId(0x157);
/// The header's `D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDPLANESLICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDPLANESLICE: MessageId =
    MessageId(0x158);
/// The header's `D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDIMENSIONS: MessageId =
    MessageId(0x159);
/// The header's `D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_UNRECOGNIZEDFORMAT: MessageId =
    MessageId(0x15a);
/// The header's `D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDFLAGS: MessageId = MessageId(0x162);
/// The header's `D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDFORCEDSAMPLECOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDFORCEDSAMPLECOUNT: MessageId =
    MessageId(0x191);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDLOGICOPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_INVALIDLOGICOPS: MessageId = MessageId(0x193);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x19a);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEHULLSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEHULLSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x19c);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x19e);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x1a0);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId = MessageId(0x1a2);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x1a4);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x1a6);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_UAVSNOTSUPPORTED: MessageId = MessageId(0x1a9);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEHULLSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEHULLSHADER_UAVSNOTSUPPORTED: MessageId = MessageId(0x1aa);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_UAVSNOTSUPPORTED: MessageId = MessageId(0x1ab);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x1ac);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x1ad);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_UAVSNOTSUPPORTED: MessageId = MessageId(0x1ae);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x1af);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CLEARVIEW_INVALIDSOURCERECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CLEARVIEW_INVALIDSOURCERECT: MessageId = MessageId(0x1bf);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CLEARVIEW_EMPTYRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CLEARVIEW_EMPTYRECT: MessageId = MessageId(0x1c0);
/// The header's `D3D12_MESSAGE_ID_UPDATETILEMAPPINGS_INVALID_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UPDATETILEMAPPINGS_INVALID_PARAMETER: MessageId = MessageId(0x1ed);
/// The header's `D3D12_MESSAGE_ID_COPYTILEMAPPINGS_INVALID_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTILEMAPPINGS_INVALID_PARAMETER: MessageId = MessageId(0x1ee);
/// The header's `D3D12_MESSAGE_ID_CREATEDEVICE_INVALIDARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEVICE_INVALIDARGS: MessageId = MessageId(0x1fa);
/// The header's `D3D12_MESSAGE_ID_CREATEDEVICE_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEVICE_WARNING: MessageId = MessageId(0x1fb);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_TYPE: MessageId = MessageId(0x207);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_NULL_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_NULL_POINTER: MessageId = MessageId(0x208);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_SUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_SUBRESOURCE: MessageId = MessageId(0x209);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_RESERVED_BITS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_RESERVED_BITS: MessageId = MessageId(0x20a);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_MISSING_BIND_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_MISSING_BIND_FLAGS: MessageId = MessageId(0x20b);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_MISMATCHING_MISC_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_MISMATCHING_MISC_FLAGS: MessageId = MessageId(0x20c);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_MATCHING_STATES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_MATCHING_STATES: MessageId = MessageId(0x20d);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_COMBINATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_COMBINATION: MessageId = MessageId(0x20e);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_BEFORE_AFTER_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_BEFORE_AFTER_MISMATCH: MessageId = MessageId(0x20f);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_RESOURCE: MessageId = MessageId(0x210);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_SAMPLE_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_SAMPLE_COUNT: MessageId = MessageId(0x211);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_FLAGS: MessageId = MessageId(0x212);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_COMBINED_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_COMBINED_FLAGS: MessageId = MessageId(0x213);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_FLAGS_FOR_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_FLAGS_FOR_FORMAT: MessageId = MessageId(0x214);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_SPLIT_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_SPLIT_BARRIER: MessageId = MessageId(0x215);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_UNMATCHED_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_UNMATCHED_END: MessageId = MessageId(0x216);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_UNMATCHED_BEGIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_UNMATCHED_BEGIN: MessageId = MessageId(0x217);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_FLAG: MessageId = MessageId(0x218);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_COMMAND_LIST_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_COMMAND_LIST_TYPE: MessageId = MessageId(0x219);
/// The header's `D3D12_MESSAGE_ID_INVALID_SUBRESOURCE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INVALID_SUBRESOURCE_STATE: MessageId = MessageId(0x21a);
/// The header's `D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_CONTENTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_CONTENTION: MessageId = MessageId(0x21c);
/// The header's `D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_RESET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_RESET: MessageId = MessageId(0x21d);
/// The header's `D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_RESET_BUNDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_RESET_BUNDLE: MessageId = MessageId(0x21e);
/// The header's `D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_CANNOT_RESET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_CANNOT_RESET: MessageId = MessageId(0x21f);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_OPEN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_OPEN: MessageId = MessageId(0x220);
/// The header's `D3D12_MESSAGE_ID_INVALID_BUNDLE_API`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INVALID_BUNDLE_API: MessageId = MessageId(0x222);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_CLOSED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_CLOSED: MessageId = MessageId(0x223);
/// The header's `D3D12_MESSAGE_ID_WRONG_COMMAND_ALLOCATOR_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRONG_COMMAND_ALLOCATOR_TYPE: MessageId = MessageId(0x225);
/// The header's `D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_SYNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_SYNC: MessageId = MessageId(0x228);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_SYNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_SYNC: MessageId = MessageId(0x229);
/// The header's `D3D12_MESSAGE_ID_SET_DESCRIPTOR_HEAP_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_DESCRIPTOR_HEAP_INVALID: MessageId = MessageId(0x22a);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMANDQUEUE: MessageId = MessageId(0x22d);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMANDALLOCATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMANDALLOCATOR: MessageId = MessageId(0x22e);
/// The header's `D3D12_MESSAGE_ID_CREATE_PIPELINESTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_PIPELINESTATE: MessageId = MessageId(0x22f);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMANDLIST12`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMANDLIST12: MessageId = MessageId(0x230);
/// The header's `D3D12_MESSAGE_ID_CREATE_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_RESOURCE: MessageId = MessageId(0x232);
/// The header's `D3D12_MESSAGE_ID_CREATE_DESCRIPTORHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_DESCRIPTORHEAP: MessageId = MessageId(0x233);
/// The header's `D3D12_MESSAGE_ID_CREATE_ROOTSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_ROOTSIGNATURE: MessageId = MessageId(0x234);
/// The header's `D3D12_MESSAGE_ID_CREATE_LIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_LIBRARY: MessageId = MessageId(0x235);
/// The header's `D3D12_MESSAGE_ID_CREATE_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_HEAP: MessageId = MessageId(0x236);
/// The header's `D3D12_MESSAGE_ID_CREATE_MONITOREDFENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_MONITOREDFENCE: MessageId = MessageId(0x237);
/// The header's `D3D12_MESSAGE_ID_CREATE_QUERYHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_QUERYHEAP: MessageId = MessageId(0x238);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMANDSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMANDSIGNATURE: MessageId = MessageId(0x239);
/// The header's `D3D12_MESSAGE_ID_LIVE_COMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_COMMANDQUEUE: MessageId = MessageId(0x23a);
/// The header's `D3D12_MESSAGE_ID_LIVE_COMMANDALLOCATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_COMMANDALLOCATOR: MessageId = MessageId(0x23b);
/// The header's `D3D12_MESSAGE_ID_LIVE_PIPELINESTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_PIPELINESTATE: MessageId = MessageId(0x23c);
/// The header's `D3D12_MESSAGE_ID_LIVE_COMMANDLIST12`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_COMMANDLIST12: MessageId = MessageId(0x23d);
/// The header's `D3D12_MESSAGE_ID_LIVE_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_RESOURCE: MessageId = MessageId(0x23f);
/// The header's `D3D12_MESSAGE_ID_LIVE_DESCRIPTORHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_DESCRIPTORHEAP: MessageId = MessageId(0x240);
/// The header's `D3D12_MESSAGE_ID_LIVE_ROOTSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_ROOTSIGNATURE: MessageId = MessageId(0x241);
/// The header's `D3D12_MESSAGE_ID_LIVE_LIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_LIBRARY: MessageId = MessageId(0x242);
/// The header's `D3D12_MESSAGE_ID_LIVE_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_HEAP: MessageId = MessageId(0x243);
/// The header's `D3D12_MESSAGE_ID_LIVE_MONITOREDFENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_MONITOREDFENCE: MessageId = MessageId(0x244);
/// The header's `D3D12_MESSAGE_ID_LIVE_QUERYHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_QUERYHEAP: MessageId = MessageId(0x245);
/// The header's `D3D12_MESSAGE_ID_LIVE_COMMANDSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_COMMANDSIGNATURE: MessageId = MessageId(0x246);
/// The header's `D3D12_MESSAGE_ID_DESTROY_COMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_COMMANDQUEUE: MessageId = MessageId(0x247);
/// The header's `D3D12_MESSAGE_ID_DESTROY_COMMANDALLOCATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_COMMANDALLOCATOR: MessageId = MessageId(0x248);
/// The header's `D3D12_MESSAGE_ID_DESTROY_PIPELINESTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_PIPELINESTATE: MessageId = MessageId(0x249);
/// The header's `D3D12_MESSAGE_ID_DESTROY_COMMANDLIST12`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_COMMANDLIST12: MessageId = MessageId(0x24a);
/// The header's `D3D12_MESSAGE_ID_DESTROY_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_RESOURCE: MessageId = MessageId(0x24c);
/// The header's `D3D12_MESSAGE_ID_DESTROY_DESCRIPTORHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_DESCRIPTORHEAP: MessageId = MessageId(0x24d);
/// The header's `D3D12_MESSAGE_ID_DESTROY_ROOTSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_ROOTSIGNATURE: MessageId = MessageId(0x24e);
/// The header's `D3D12_MESSAGE_ID_DESTROY_LIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_LIBRARY: MessageId = MessageId(0x24f);
/// The header's `D3D12_MESSAGE_ID_DESTROY_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_HEAP: MessageId = MessageId(0x250);
/// The header's `D3D12_MESSAGE_ID_DESTROY_MONITOREDFENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_MONITOREDFENCE: MessageId = MessageId(0x251);
/// The header's `D3D12_MESSAGE_ID_DESTROY_QUERYHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_QUERYHEAP: MessageId = MessageId(0x252);
/// The header's `D3D12_MESSAGE_ID_DESTROY_COMMANDSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_COMMANDSIGNATURE: MessageId = MessageId(0x253);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDDIMENSIONS: MessageId = MessageId(0x255);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDMISCFLAGS: MessageId = MessageId(0x257);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDARG_RETURN: MessageId = MessageId(0x25a);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_OUTOFMEMORY_RETURN: MessageId = MessageId(0x25b);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDDESC: MessageId = MessageId(0x25c);
/// The header's `D3D12_MESSAGE_ID_POSSIBLY_INVALID_SUBRESOURCE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_POSSIBLY_INVALID_SUBRESOURCE_STATE: MessageId = MessageId(0x25f);
/// The header's `D3D12_MESSAGE_ID_INVALID_USE_OF_NON_RESIDENT_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INVALID_USE_OF_NON_RESIDENT_RESOURCE: MessageId = MessageId(0x260);
/// The header's `D3D12_MESSAGE_ID_POSSIBLE_INVALID_USE_OF_NON_RESIDENT_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_POSSIBLE_INVALID_USE_OF_NON_RESIDENT_RESOURCE: MessageId =
    MessageId(0x261);
/// The header's `D3D12_MESSAGE_ID_BUNDLE_PIPELINE_STATE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_BUNDLE_PIPELINE_STATE_MISMATCH: MessageId = MessageId(0x262);
/// The header's `D3D12_MESSAGE_ID_PRIMITIVE_TOPOLOGY_MISMATCH_PIPELINE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PRIMITIVE_TOPOLOGY_MISMATCH_PIPELINE_STATE: MessageId = MessageId(0x263);
/// The header's `D3D12_MESSAGE_ID_RENDER_TARGET_FORMAT_MISMATCH_PIPELINE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_TARGET_FORMAT_MISMATCH_PIPELINE_STATE: MessageId =
    MessageId(0x265);
/// The header's `D3D12_MESSAGE_ID_RENDER_TARGET_SAMPLE_DESC_MISMATCH_PIPELINE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_TARGET_SAMPLE_DESC_MISMATCH_PIPELINE_STATE: MessageId =
    MessageId(0x266);
/// The header's `D3D12_MESSAGE_ID_DEPTH_STENCIL_FORMAT_MISMATCH_PIPELINE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEPTH_STENCIL_FORMAT_MISMATCH_PIPELINE_STATE: MessageId =
    MessageId(0x267);
/// The header's `D3D12_MESSAGE_ID_DEPTH_STENCIL_SAMPLE_DESC_MISMATCH_PIPELINE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEPTH_STENCIL_SAMPLE_DESC_MISMATCH_PIPELINE_STATE: MessageId =
    MessageId(0x268);
/// The header's `D3D12_MESSAGE_ID_CREATESHADER_INVALIDBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADER_INVALIDBYTECODE: MessageId = MessageId(0x26e);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_NULLDESC: MessageId = MessageId(0x26f);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_INVALIDSIZE: MessageId = MessageId(0x270);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_UNRECOGNIZEDHEAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_UNRECOGNIZEDHEAPTYPE: MessageId = MessageId(0x271);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_UNRECOGNIZEDCPUPAGEPROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_UNRECOGNIZEDCPUPAGEPROPERTIES: MessageId = MessageId(0x272);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_UNRECOGNIZEDMEMORYPOOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_UNRECOGNIZEDMEMORYPOOL: MessageId = MessageId(0x273);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_INVALIDPROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_INVALIDPROPERTIES: MessageId = MessageId(0x274);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_INVALIDALIGNMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_INVALIDALIGNMENT: MessageId = MessageId(0x275);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_UNRECOGNIZEDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_UNRECOGNIZEDMISCFLAGS: MessageId = MessageId(0x276);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_INVALIDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_INVALIDMISCFLAGS: MessageId = MessageId(0x277);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_INVALIDARG_RETURN: MessageId = MessageId(0x278);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_OUTOFMEMORY_RETURN: MessageId = MessageId(0x279);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_NULLHEAPPROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_NULLHEAPPROPERTIES: MessageId = MessageId(0x27a);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_UNRECOGNIZEDHEAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_UNRECOGNIZEDHEAPTYPE: MessageId = MessageId(0x27b);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_UNRECOGNIZEDCPUPAGEPROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_UNRECOGNIZEDCPUPAGEPROPERTIES: MessageId =
    MessageId(0x27c);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_UNRECOGNIZEDMEMORYPOOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_UNRECOGNIZEDMEMORYPOOL: MessageId =
    MessageId(0x27d);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALIDHEAPPROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALIDHEAPPROPERTIES: MessageId =
    MessageId(0x27e);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_UNRECOGNIZEDHEAPMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_UNRECOGNIZEDHEAPMISCFLAGS: MessageId =
    MessageId(0x27f);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALIDHEAPMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALIDHEAPMISCFLAGS: MessageId = MessageId(0x280);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALIDARG_RETURN: MessageId = MessageId(0x281);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_OUTOFMEMORY_RETURN: MessageId = MessageId(0x282);
/// The header's `D3D12_MESSAGE_ID_GETCUSTOMHEAPPROPERTIES_UNRECOGNIZEDHEAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETCUSTOMHEAPPROPERTIES_UNRECOGNIZEDHEAPTYPE: MessageId =
    MessageId(0x283);
/// The header's `D3D12_MESSAGE_ID_GETCUSTOMHEAPPROPERTIES_INVALIDHEAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETCUSTOMHEAPPROPERTIES_INVALIDHEAPTYPE: MessageId = MessageId(0x284);
/// The header's `D3D12_MESSAGE_ID_CREATE_DESCRIPTOR_HEAP_INVALID_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_DESCRIPTOR_HEAP_INVALID_DESC: MessageId = MessageId(0x285);
/// The header's `D3D12_MESSAGE_ID_INVALID_DESCRIPTOR_HANDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INVALID_DESCRIPTOR_HANDLE: MessageId = MessageId(0x286);
/// The header's `D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALID_CONSERVATIVERASTERMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALID_CONSERVATIVERASTERMODE: MessageId =
    MessageId(0x287);
/// The header's `D3D12_MESSAGE_ID_CREATE_CONSTANT_BUFFER_VIEW_INVALID_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_CONSTANT_BUFFER_VIEW_INVALID_RESOURCE: MessageId =
    MessageId(0x289);
/// The header's `D3D12_MESSAGE_ID_CREATE_CONSTANT_BUFFER_VIEW_INVALID_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_CONSTANT_BUFFER_VIEW_INVALID_DESC: MessageId = MessageId(0x28a);
/// The header's `D3D12_MESSAGE_ID_CREATE_UNORDEREDACCESS_VIEW_INVALID_COUNTER_USAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_UNORDEREDACCESS_VIEW_INVALID_COUNTER_USAGE: MessageId =
    MessageId(0x28c);
/// The header's `D3D12_MESSAGE_ID_COPY_DESCRIPTORS_INVALID_RANGES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPY_DESCRIPTORS_INVALID_RANGES: MessageId = MessageId(0x28d);
/// The header's `D3D12_MESSAGE_ID_COPY_DESCRIPTORS_WRITE_ONLY_DESCRIPTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPY_DESCRIPTORS_WRITE_ONLY_DESCRIPTOR: MessageId = MessageId(0x28e);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_RTV_FORMAT_NOT_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_RTV_FORMAT_NOT_UNKNOWN: MessageId =
    MessageId(0x28f);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_RENDER_TARGET_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_RENDER_TARGET_COUNT: MessageId =
    MessageId(0x290);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_VERTEX_SHADER_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_VERTEX_SHADER_NOT_SET: MessageId =
    MessageId(0x291);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INPUTLAYOUT_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INPUTLAYOUT_NOT_SET: MessageId =
    MessageId(0x292);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_HS_DS_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_HS_DS_SIGNATURE_MISMATCH:
    MessageId = MessageId(0x293);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_REGISTERINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_REGISTERINDEX: MessageId =
    MessageId(0x294);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_COMPONENTTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_COMPONENTTYPE: MessageId =
    MessageId(0x295);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_REGISTERMASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_REGISTERMASK: MessageId =
    MessageId(0x296);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_SYSTEMVALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_SYSTEMVALUE: MessageId =
    MessageId(0x297);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_NEVERWRITTEN_ALWAYSREADS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_NEVERWRITTEN_ALWAYSREADS:
    MessageId = MessageId(0x298);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_MINPRECISION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_MINPRECISION: MessageId =
    MessageId(0x299);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_SEMANTICNAME_NOT_FOUND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_LINKAGE_SEMANTICNAME_NOT_FOUND:
    MessageId = MessageId(0x29a);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HS_XOR_DS_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HS_XOR_DS_MISMATCH: MessageId =
    MessageId(0x29b);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HULL_SHADER_INPUT_TOPOLOGY_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HULL_SHADER_INPUT_TOPOLOGY_MISMATCH:
    MessageId = MessageId(0x29c);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HS_DS_CONTROL_POINT_COUNT_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HS_DS_CONTROL_POINT_COUNT_MISMATCH:
    MessageId = MessageId(0x29d);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HS_DS_TESSELLATOR_DOMAIN_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HS_DS_TESSELLATOR_DOMAIN_MISMATCH:
    MessageId = MessageId(0x29e);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_USE_OF_CENTER_MULTISAMPLE_PATTERN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_USE_OF_CENTER_MULTISAMPLE_PATTERN:
    MessageId = MessageId(0x29f);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_USE_OF_FORCED_SAMPLE_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_USE_OF_FORCED_SAMPLE_COUNT:
    MessageId = MessageId(0x2a0);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_PRIMITIVETOPOLOGY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_PRIMITIVETOPOLOGY: MessageId =
    MessageId(0x2a1);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_SYSTEMVALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_SYSTEMVALUE: MessageId =
    MessageId(0x2a2);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_OM_DUAL_SOURCE_BLENDING_CAN_ONLY_HAVE_RENDER_TARGET_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_OM_DUAL_SOURCE_BLENDING_CAN_ONLY_HAVE_RENDER_TARGET_0: MessageId = MessageId(0x2a3);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_OM_RENDER_TARGET_DOES_NOT_SUPPORT_BLENDING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_OM_RENDER_TARGET_DOES_NOT_SUPPORT_BLENDING: MessageId = MessageId(0x2a4);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_PS_OUTPUT_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_PS_OUTPUT_TYPE_MISMATCH: MessageId =
    MessageId(0x2a5);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_OM_RENDER_TARGET_DOES_NOT_SUPPORT_LOGIC_OPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_OM_RENDER_TARGET_DOES_NOT_SUPPORT_LOGIC_OPS: MessageId = MessageId(0x2a6);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_RENDERTARGETVIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_RENDERTARGETVIEW_NOT_SET: MessageId =
    MessageId(0x2a7);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_DEPTHSTENCILVIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_DEPTHSTENCILVIEW_NOT_SET: MessageId =
    MessageId(0x2a8);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_GS_INPUT_PRIMITIVE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_GS_INPUT_PRIMITIVE_MISMATCH: MessageId =
    MessageId(0x2a9);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_POSITION_NOT_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_POSITION_NOT_PRESENT: MessageId =
    MessageId(0x2aa);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MISSING_ROOT_SIGNATURE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MISSING_ROOT_SIGNATURE_FLAGS: MessageId =
    MessageId(0x2ab);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_INDEX_BUFFER_PROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_INDEX_BUFFER_PROPERTIES: MessageId =
    MessageId(0x2ac);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_SAMPLE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INVALID_SAMPLE_DESC: MessageId =
    MessageId(0x2ad);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HS_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_HS_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x2ae);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_DS_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_DS_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x2af);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_VS_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_VS_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x2b0);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_GS_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_GS_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x2b1);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_PS_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_PS_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x2b2);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MISSING_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MISSING_ROOT_SIGNATURE: MessageId =
    MessageId(0x2b3);
/// The header's `D3D12_MESSAGE_ID_EXECUTE_BUNDLE_OPEN_BUNDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTE_BUNDLE_OPEN_BUNDLE: MessageId = MessageId(0x2b4);
/// The header's `D3D12_MESSAGE_ID_EXECUTE_BUNDLE_DESCRIPTOR_HEAP_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTE_BUNDLE_DESCRIPTOR_HEAP_MISMATCH: MessageId = MessageId(0x2b5);
/// The header's `D3D12_MESSAGE_ID_EXECUTE_BUNDLE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTE_BUNDLE_TYPE: MessageId = MessageId(0x2b6);
/// The header's `D3D12_MESSAGE_ID_DRAW_EMPTY_SCISSOR_RECTANGLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DRAW_EMPTY_SCISSOR_RECTANGLE: MessageId = MessageId(0x2b7);
/// The header's `D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_BLOB_NOT_FOUND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_BLOB_NOT_FOUND: MessageId = MessageId(0x2b8);
/// The header's `D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_DESERIALIZE_FAILED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_DESERIALIZE_FAILED: MessageId = MessageId(0x2b9);
/// The header's `D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_INVALID_CONFIGURATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_INVALID_CONFIGURATION: MessageId =
    MessageId(0x2ba);
/// The header's `D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_NOT_SUPPORTED_ON_DEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_NOT_SUPPORTED_ON_DEVICE: MessageId =
    MessageId(0x2bb);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_NULLRESOURCEPROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_NULLRESOURCEPROPERTIES: MessageId =
    MessageId(0x2bc);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_NULLHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_NULLHEAP: MessageId = MessageId(0x2bd);
/// The header's `D3D12_MESSAGE_ID_GETRESOURCEALLOCATIONINFO_INVALIDRDESCS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETRESOURCEALLOCATIONINFO_INVALIDRDESCS: MessageId = MessageId(0x2be);
/// The header's `D3D12_MESSAGE_ID_MAKERESIDENT_NULLOBJECTARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAKERESIDENT_NULLOBJECTARRAY: MessageId = MessageId(0x2bf);
/// The header's `D3D12_MESSAGE_ID_EVICT_NULLOBJECTARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EVICT_NULLOBJECTARRAY: MessageId = MessageId(0x2c1);
/// The header's `D3D12_MESSAGE_ID_SET_DESCRIPTOR_TABLE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_DESCRIPTOR_TABLE_INVALID: MessageId = MessageId(0x2c4);
/// The header's `D3D12_MESSAGE_ID_SET_ROOT_CONSTANT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_ROOT_CONSTANT_INVALID: MessageId = MessageId(0x2c5);
/// The header's `D3D12_MESSAGE_ID_SET_ROOT_CONSTANT_BUFFER_VIEW_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_ROOT_CONSTANT_BUFFER_VIEW_INVALID: MessageId = MessageId(0x2c6);
/// The header's `D3D12_MESSAGE_ID_SET_ROOT_SHADER_RESOURCE_VIEW_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_ROOT_SHADER_RESOURCE_VIEW_INVALID: MessageId = MessageId(0x2c7);
/// The header's `D3D12_MESSAGE_ID_SET_ROOT_UNORDERED_ACCESS_VIEW_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_ROOT_UNORDERED_ACCESS_VIEW_INVALID: MessageId = MessageId(0x2c8);
/// The header's `D3D12_MESSAGE_ID_SET_VERTEX_BUFFERS_INVALID_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_VERTEX_BUFFERS_INVALID_DESC: MessageId = MessageId(0x2c9);
/// The header's `D3D12_MESSAGE_ID_SET_INDEX_BUFFER_INVALID_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_INDEX_BUFFER_INVALID_DESC: MessageId = MessageId(0x2cb);
/// The header's `D3D12_MESSAGE_ID_SET_STREAM_OUTPUT_BUFFERS_INVALID_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_STREAM_OUTPUT_BUFFERS_INVALID_DESC: MessageId = MessageId(0x2cd);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_UNRECOGNIZEDDIMENSIONALITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_UNRECOGNIZEDDIMENSIONALITY: MessageId = MessageId(0x2ce);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_UNRECOGNIZEDLAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_UNRECOGNIZEDLAYOUT: MessageId = MessageId(0x2cf);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDDIMENSIONALITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDDIMENSIONALITY: MessageId = MessageId(0x2d0);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDALIGNMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDALIGNMENT: MessageId = MessageId(0x2d1);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDMIPLEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDMIPLEVELS: MessageId = MessageId(0x2d2);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDSAMPLEDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDSAMPLEDESC: MessageId = MessageId(0x2d3);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDLAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDLAYOUT: MessageId = MessageId(0x2d4);
/// The header's `D3D12_MESSAGE_ID_SET_INDEX_BUFFER_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_INDEX_BUFFER_INVALID: MessageId = MessageId(0x2d5);
/// The header's `D3D12_MESSAGE_ID_SET_VERTEX_BUFFERS_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_VERTEX_BUFFERS_INVALID: MessageId = MessageId(0x2d6);
/// The header's `D3D12_MESSAGE_ID_SET_STREAM_OUTPUT_BUFFERS_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_STREAM_OUTPUT_BUFFERS_INVALID: MessageId = MessageId(0x2d7);
/// The header's `D3D12_MESSAGE_ID_SET_RENDER_TARGETS_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_RENDER_TARGETS_INVALID: MessageId = MessageId(0x2d8);
/// The header's `D3D12_MESSAGE_ID_CREATEQUERY_HEAP_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEQUERY_HEAP_INVALID_PARAMETERS: MessageId = MessageId(0x2d9);
/// The header's `D3D12_MESSAGE_ID_BEGIN_END_QUERY_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_BEGIN_END_QUERY_INVALID_PARAMETERS: MessageId = MessageId(0x2db);
/// The header's `D3D12_MESSAGE_ID_CLOSE_COMMAND_LIST_OPEN_QUERY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CLOSE_COMMAND_LIST_OPEN_QUERY: MessageId = MessageId(0x2dc);
/// The header's `D3D12_MESSAGE_ID_RESOLVE_QUERY_DATA_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVE_QUERY_DATA_INVALID_PARAMETERS: MessageId = MessageId(0x2dd);
/// The header's `D3D12_MESSAGE_ID_SET_PREDICATION_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_PREDICATION_INVALID_PARAMETERS: MessageId = MessageId(0x2de);
/// The header's `D3D12_MESSAGE_ID_TIMESTAMPS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_TIMESTAMPS_NOT_SUPPORTED: MessageId = MessageId(0x2df);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_UNRECOGNIZEDFORMAT: MessageId = MessageId(0x2e1);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDFORMAT: MessageId = MessageId(0x2e2);
/// The header's `D3D12_MESSAGE_ID_GETCOPYABLEFOOTPRINTS_INVALIDSUBRESOURCERANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETCOPYABLEFOOTPRINTS_INVALIDSUBRESOURCERANGE: MessageId =
    MessageId(0x2e3);
/// The header's `D3D12_MESSAGE_ID_GETCOPYABLEFOOTPRINTS_INVALIDBASEOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETCOPYABLEFOOTPRINTS_INVALIDBASEOFFSET: MessageId = MessageId(0x2e4);
/// The header's `D3D12_MESSAGE_ID_GETCOPYABLELAYOUT_INVALIDSUBRESOURCERANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETCOPYABLELAYOUT_INVALIDSUBRESOURCERANGE: MessageId = MessageId(0x2e3);
/// The header's `D3D12_MESSAGE_ID_GETCOPYABLELAYOUT_INVALIDBASEOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETCOPYABLELAYOUT_INVALIDBASEOFFSET: MessageId = MessageId(0x2e4);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_HEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_INVALID_HEAP: MessageId = MessageId(0x2e5);
/// The header's `D3D12_MESSAGE_ID_CREATE_SAMPLER_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_SAMPLER_INVALID: MessageId = MessageId(0x2e6);
/// The header's `D3D12_MESSAGE_ID_CREATECOMMANDSIGNATURE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATECOMMANDSIGNATURE_INVALID: MessageId = MessageId(0x2e7);
/// The header's `D3D12_MESSAGE_ID_EXECUTE_INDIRECT_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTE_INDIRECT_INVALID_PARAMETERS: MessageId = MessageId(0x2e8);
/// The header's `D3D12_MESSAGE_ID_GETGPUVIRTUALADDRESS_INVALID_RESOURCE_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETGPUVIRTUALADDRESS_INVALID_RESOURCE_DIMENSION: MessageId =
    MessageId(0x2e9);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDCLEARVALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDCLEARVALUE: MessageId = MessageId(0x32f);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_UNRECOGNIZEDCLEARVALUEFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_UNRECOGNIZEDCLEARVALUEFORMAT: MessageId =
    MessageId(0x330);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDCLEARVALUEFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDCLEARVALUEFORMAT: MessageId = MessageId(0x331);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_CLEARVALUEDENORMFLUSH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_CLEARVALUEDENORMFLUSH: MessageId = MessageId(0x332);
/// The header's `D3D12_MESSAGE_ID_CLEARRENDERTARGETVIEW_MISMATCHINGCLEARVALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CLEARRENDERTARGETVIEW_MISMATCHINGCLEARVALUE: MessageId =
    MessageId(0x334);
/// The header's `D3D12_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_MISMATCHINGCLEARVALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_MISMATCHINGCLEARVALUE: MessageId =
    MessageId(0x335);
/// The header's `D3D12_MESSAGE_ID_MAP_INVALIDHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAP_INVALIDHEAP: MessageId = MessageId(0x336);
/// The header's `D3D12_MESSAGE_ID_UNMAP_INVALIDHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNMAP_INVALIDHEAP: MessageId = MessageId(0x337);
/// The header's `D3D12_MESSAGE_ID_MAP_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAP_INVALIDRESOURCE: MessageId = MessageId(0x338);
/// The header's `D3D12_MESSAGE_ID_UNMAP_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNMAP_INVALIDRESOURCE: MessageId = MessageId(0x339);
/// The header's `D3D12_MESSAGE_ID_MAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x33a);
/// The header's `D3D12_MESSAGE_ID_UNMAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNMAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x33b);
/// The header's `D3D12_MESSAGE_ID_MAP_INVALIDRANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAP_INVALIDRANGE: MessageId = MessageId(0x33c);
/// The header's `D3D12_MESSAGE_ID_UNMAP_INVALIDRANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNMAP_INVALIDRANGE: MessageId = MessageId(0x33d);
/// The header's `D3D12_MESSAGE_ID_MAP_INVALIDDATAPOINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAP_INVALIDDATAPOINTER: MessageId = MessageId(0x340);
/// The header's `D3D12_MESSAGE_ID_MAP_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAP_INVALIDARG_RETURN: MessageId = MessageId(0x341);
/// The header's `D3D12_MESSAGE_ID_MAP_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAP_OUTOFMEMORY_RETURN: MessageId = MessageId(0x342);
/// The header's `D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_BUNDLENOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_BUNDLENOTSUPPORTED: MessageId = MessageId(0x343);
/// The header's `D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_COMMANDLISTMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_COMMANDLISTMISMATCH: MessageId = MessageId(0x344);
/// The header's `D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_OPENCOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_OPENCOMMANDLIST: MessageId = MessageId(0x345);
/// The header's `D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_FAILEDCOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_FAILEDCOMMANDLIST: MessageId = MessageId(0x346);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_NULLDST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_NULLDST: MessageId = MessageId(0x347);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALIDDSTRESOURCEDIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALIDDSTRESOURCEDIMENSION: MessageId =
    MessageId(0x348);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_DSTRANGEOUTOFBOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_DSTRANGEOUTOFBOUNDS: MessageId = MessageId(0x349);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_NULLSRC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_NULLSRC: MessageId = MessageId(0x34a);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALIDSRCRESOURCEDIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALIDSRCRESOURCEDIMENSION: MessageId =
    MessageId(0x34b);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_SRCRANGEOUTOFBOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_SRCRANGEOUTOFBOUNDS: MessageId = MessageId(0x34c);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALIDCOPYFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALIDCOPYFLAGS: MessageId = MessageId(0x34d);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_NULLDST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_NULLDST: MessageId = MessageId(0x34e);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_UNRECOGNIZEDDSTTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_UNRECOGNIZEDDSTTYPE: MessageId = MessageId(0x34f);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTRESOURCEDIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTRESOURCEDIMENSION: MessageId =
    MessageId(0x350);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTRESOURCE: MessageId = MessageId(0x351);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTSUBRESOURCE: MessageId = MessageId(0x352);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTOFFSET: MessageId = MessageId(0x353);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_UNRECOGNIZEDDSTFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_UNRECOGNIZEDDSTFORMAT: MessageId = MessageId(0x354);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTFORMAT: MessageId = MessageId(0x355);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTDIMENSIONS: MessageId = MessageId(0x356);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTROWPITCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTROWPITCH: MessageId = MessageId(0x357);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTPLACEMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTPLACEMENT: MessageId = MessageId(0x358);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTDSPLACEDFOOTPRINTFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTDSPLACEDFOOTPRINTFORMAT: MessageId =
    MessageId(0x359);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_DSTREGIONOUTOFBOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_DSTREGIONOUTOFBOUNDS: MessageId = MessageId(0x35a);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_NULLSRC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_NULLSRC: MessageId = MessageId(0x35b);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_UNRECOGNIZEDSRCTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_UNRECOGNIZEDSRCTYPE: MessageId = MessageId(0x35c);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCRESOURCEDIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCRESOURCEDIMENSION: MessageId =
    MessageId(0x35d);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCRESOURCE: MessageId = MessageId(0x35e);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCSUBRESOURCE: MessageId = MessageId(0x35f);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCOFFSET: MessageId = MessageId(0x360);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_UNRECOGNIZEDSRCFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_UNRECOGNIZEDSRCFORMAT: MessageId = MessageId(0x361);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCFORMAT: MessageId = MessageId(0x362);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCDIMENSIONS: MessageId = MessageId(0x363);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCROWPITCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCROWPITCH: MessageId = MessageId(0x364);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCPLACEMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCPLACEMENT: MessageId = MessageId(0x365);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCDSPLACEDFOOTPRINTFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCDSPLACEDFOOTPRINTFORMAT: MessageId =
    MessageId(0x366);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_SRCREGIONOUTOFBOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_SRCREGIONOUTOFBOUNDS: MessageId = MessageId(0x367);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTCOORDINATES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDDSTCOORDINATES: MessageId = MessageId(0x368);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDSRCBOX: MessageId = MessageId(0x369);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_FORMATMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_FORMATMISMATCH: MessageId = MessageId(0x36a);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_EMPTYBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_EMPTYBOX: MessageId = MessageId(0x36b);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDCOPYFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_INVALIDCOPYFLAGS: MessageId = MessageId(0x36c);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALID_SUBRESOURCE_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALID_SUBRESOURCE_INDEX: MessageId =
    MessageId(0x36d);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALID_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALID_FORMAT: MessageId = MessageId(0x36e);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_RESOURCE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_RESOURCE_MISMATCH: MessageId = MessageId(0x36f);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALID_SAMPLE_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALID_SAMPLE_COUNT: MessageId = MessageId(0x370);
/// The header's `D3D12_MESSAGE_ID_CREATECOMPUTEPIPELINESTATE_INVALID_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATECOMPUTEPIPELINESTATE_INVALID_SHADER: MessageId = MessageId(0x371);
/// The header's `D3D12_MESSAGE_ID_CREATECOMPUTEPIPELINESTATE_CS_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATECOMPUTEPIPELINESTATE_CS_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x372);
/// The header's `D3D12_MESSAGE_ID_CREATECOMPUTEPIPELINESTATE_MISSING_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATECOMPUTEPIPELINESTATE_MISSING_ROOT_SIGNATURE: MessageId =
    MessageId(0x373);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_INVALIDCACHEDBLOB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_INVALIDCACHEDBLOB: MessageId = MessageId(0x374);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CACHEDBLOBADAPTERMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CACHEDBLOBADAPTERMISMATCH: MessageId =
    MessageId(0x375);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CACHEDBLOBDRIVERVERSIONMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CACHEDBLOBDRIVERVERSIONMISMATCH: MessageId =
    MessageId(0x376);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CACHEDBLOBDESCMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CACHEDBLOBDESCMISMATCH: MessageId = MessageId(0x377);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CACHEDBLOBIGNORED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CACHEDBLOBIGNORED: MessageId = MessageId(0x378);
/// The header's `D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_INVALIDHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_INVALIDHEAP: MessageId = MessageId(0x379);
/// The header's `D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_INVALIDRESOURCE: MessageId = MessageId(0x37a);
/// The header's `D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_INVALIDBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_INVALIDBOX: MessageId = MessageId(0x37b);
/// The header's `D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_INVALIDSUBRESOURCE: MessageId = MessageId(0x37c);
/// The header's `D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_EMPTYBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITETOSUBRESOURCE_EMPTYBOX: MessageId = MessageId(0x37d);
/// The header's `D3D12_MESSAGE_ID_READFROMSUBRESOURCE_INVALIDHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_READFROMSUBRESOURCE_INVALIDHEAP: MessageId = MessageId(0x37e);
/// The header's `D3D12_MESSAGE_ID_READFROMSUBRESOURCE_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_READFROMSUBRESOURCE_INVALIDRESOURCE: MessageId = MessageId(0x37f);
/// The header's `D3D12_MESSAGE_ID_READFROMSUBRESOURCE_INVALIDBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_READFROMSUBRESOURCE_INVALIDBOX: MessageId = MessageId(0x380);
/// The header's `D3D12_MESSAGE_ID_READFROMSUBRESOURCE_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_READFROMSUBRESOURCE_INVALIDSUBRESOURCE: MessageId = MessageId(0x381);
/// The header's `D3D12_MESSAGE_ID_READFROMSUBRESOURCE_EMPTYBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_READFROMSUBRESOURCE_EMPTYBOX: MessageId = MessageId(0x382);
/// The header's `D3D12_MESSAGE_ID_TOO_MANY_NODES_SPECIFIED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_TOO_MANY_NODES_SPECIFIED: MessageId = MessageId(0x383);
/// The header's `D3D12_MESSAGE_ID_INVALID_NODE_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INVALID_NODE_INDEX: MessageId = MessageId(0x384);
/// The header's `D3D12_MESSAGE_ID_GETHEAPPROPERTIES_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETHEAPPROPERTIES_INVALIDRESOURCE: MessageId = MessageId(0x385);
/// The header's `D3D12_MESSAGE_ID_NODE_MASK_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_NODE_MASK_MISMATCH: MessageId = MessageId(0x386);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_OUTOFMEMORY: MessageId = MessageId(0x387);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_MULTIPLE_SWAPCHAIN_BUFFER_REFERENCES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_MULTIPLE_SWAPCHAIN_BUFFER_REFERENCES: MessageId =
    MessageId(0x388);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_TOO_MANY_SWAPCHAIN_REFERENCES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_TOO_MANY_SWAPCHAIN_REFERENCES: MessageId = MessageId(0x389);
/// The header's `D3D12_MESSAGE_ID_COMMAND_QUEUE_TOO_MANY_SWAPCHAIN_REFERENCES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_QUEUE_TOO_MANY_SWAPCHAIN_REFERENCES: MessageId =
    MessageId(0x38a);
/// The header's `D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_WRONGSWAPCHAINBUFFERREFERENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_WRONGSWAPCHAINBUFFERREFERENCE: MessageId =
    MessageId(0x38b);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_SETRENDERTARGETS_INVALIDNUMRENDERTARGETS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_SETRENDERTARGETS_INVALIDNUMRENDERTARGETS: MessageId =
    MessageId(0x38c);
/// The header's `D3D12_MESSAGE_ID_CREATE_QUEUE_INVALID_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_QUEUE_INVALID_TYPE: MessageId = MessageId(0x38d);
/// The header's `D3D12_MESSAGE_ID_CREATE_QUEUE_INVALID_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_QUEUE_INVALID_FLAGS: MessageId = MessageId(0x38e);
/// The header's `D3D12_MESSAGE_ID_CREATESHAREDRESOURCE_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHAREDRESOURCE_INVALIDFLAGS: MessageId = MessageId(0x38f);
/// The header's `D3D12_MESSAGE_ID_CREATESHAREDRESOURCE_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHAREDRESOURCE_INVALIDFORMAT: MessageId = MessageId(0x390);
/// The header's `D3D12_MESSAGE_ID_CREATESHAREDHEAP_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHAREDHEAP_INVALIDFLAGS: MessageId = MessageId(0x391);
/// The header's `D3D12_MESSAGE_ID_REFLECTSHAREDPROPERTIES_UNRECOGNIZEDPROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_REFLECTSHAREDPROPERTIES_UNRECOGNIZEDPROPERTIES: MessageId =
    MessageId(0x392);
/// The header's `D3D12_MESSAGE_ID_REFLECTSHAREDPROPERTIES_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_REFLECTSHAREDPROPERTIES_INVALIDSIZE: MessageId = MessageId(0x393);
/// The header's `D3D12_MESSAGE_ID_REFLECTSHAREDPROPERTIES_INVALIDOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_REFLECTSHAREDPROPERTIES_INVALIDOBJECT: MessageId = MessageId(0x394);
/// The header's `D3D12_MESSAGE_ID_KEYEDMUTEX_INVALIDOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_KEYEDMUTEX_INVALIDOBJECT: MessageId = MessageId(0x395);
/// The header's `D3D12_MESSAGE_ID_KEYEDMUTEX_INVALIDKEY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_KEYEDMUTEX_INVALIDKEY: MessageId = MessageId(0x396);
/// The header's `D3D12_MESSAGE_ID_KEYEDMUTEX_WRONGSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_KEYEDMUTEX_WRONGSTATE: MessageId = MessageId(0x397);
/// The header's `D3D12_MESSAGE_ID_CREATE_QUEUE_INVALID_PRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_QUEUE_INVALID_PRIORITY: MessageId = MessageId(0x398);
/// The header's `D3D12_MESSAGE_ID_OBJECT_DELETED_WHILE_STILL_IN_USE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OBJECT_DELETED_WHILE_STILL_IN_USE: MessageId = MessageId(0x399);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_INVALID_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_INVALID_FLAGS: MessageId = MessageId(0x39a);
/// The header's `D3D12_MESSAGE_ID_HEAP_ADDRESS_RANGE_HAS_NO_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_HEAP_ADDRESS_RANGE_HAS_NO_RESOURCE: MessageId = MessageId(0x39b);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_RENDER_TARGET_DELETED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_RENDER_TARGET_DELETED: MessageId = MessageId(0x39c);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_ALL_RENDER_TARGETS_HAVE_UNKNOWN_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_ALL_RENDER_TARGETS_HAVE_UNKNOWN_FORMAT:
    MessageId = MessageId(0x39d);
/// The header's `D3D12_MESSAGE_ID_HEAP_ADDRESS_RANGE_INTERSECTS_MULTIPLE_BUFFERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_HEAP_ADDRESS_RANGE_INTERSECTS_MULTIPLE_BUFFERS: MessageId =
    MessageId(0x39e);
/// The header's `D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_GPU_WRITTEN_READBACK_RESOURCE_MAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTECOMMANDLISTS_GPU_WRITTEN_READBACK_RESOURCE_MAPPED: MessageId =
    MessageId(0x39f);
/// The header's `D3D12_MESSAGE_ID_UNMAP_RANGE_NOT_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNMAP_RANGE_NOT_EMPTY: MessageId = MessageId(0x3a1);
/// The header's `D3D12_MESSAGE_ID_MAP_INVALID_NULLRANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MAP_INVALID_NULLRANGE: MessageId = MessageId(0x3a2);
/// The header's `D3D12_MESSAGE_ID_UNMAP_INVALID_NULLRANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNMAP_INVALID_NULLRANGE: MessageId = MessageId(0x3a3);
/// The header's `D3D12_MESSAGE_ID_NO_GRAPHICS_API_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_NO_GRAPHICS_API_SUPPORT: MessageId = MessageId(0x3a4);
/// The header's `D3D12_MESSAGE_ID_NO_COMPUTE_API_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_NO_COMPUTE_API_SUPPORT: MessageId = MessageId(0x3a5);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_RESOURCE_FLAGS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_RESOURCE_FLAGS_NOT_SUPPORTED: MessageId =
    MessageId(0x3a6);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_ROOT_ARGUMENT_UNINITIALIZED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_ROOT_ARGUMENT_UNINITIALIZED: MessageId =
    MessageId(0x3a7);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_DESCRIPTOR_HEAP_INDEX_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_DESCRIPTOR_HEAP_INDEX_OUT_OF_BOUNDS: MessageId =
    MessageId(0x3a8);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_DESCRIPTOR_TABLE_REGISTER_INDEX_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_DESCRIPTOR_TABLE_REGISTER_INDEX_OUT_OF_BOUNDS:
    MessageId = MessageId(0x3a9);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_DESCRIPTOR_UNINITIALIZED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_DESCRIPTOR_UNINITIALIZED: MessageId =
    MessageId(0x3aa);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_DESCRIPTOR_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_DESCRIPTOR_TYPE_MISMATCH: MessageId =
    MessageId(0x3ab);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_SRV_RESOURCE_DIMENSION_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_SRV_RESOURCE_DIMENSION_MISMATCH: MessageId =
    MessageId(0x3ac);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_UAV_RESOURCE_DIMENSION_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_UAV_RESOURCE_DIMENSION_MISMATCH: MessageId =
    MessageId(0x3ad);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_INCOMPATIBLE_RESOURCE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_INCOMPATIBLE_RESOURCE_STATE: MessageId =
    MessageId(0x3ae);
/// The header's `D3D12_MESSAGE_ID_COPYRESOURCE_NULLDST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYRESOURCE_NULLDST: MessageId = MessageId(0x3af);
/// The header's `D3D12_MESSAGE_ID_COPYRESOURCE_INVALIDDSTRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYRESOURCE_INVALIDDSTRESOURCE: MessageId = MessageId(0x3b0);
/// The header's `D3D12_MESSAGE_ID_COPYRESOURCE_NULLSRC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYRESOURCE_NULLSRC: MessageId = MessageId(0x3b1);
/// The header's `D3D12_MESSAGE_ID_COPYRESOURCE_INVALIDSRCRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYRESOURCE_INVALIDSRCRESOURCE: MessageId = MessageId(0x3b2);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_NULLDST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_NULLDST: MessageId = MessageId(0x3b3);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALIDDSTRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALIDDSTRESOURCE: MessageId = MessageId(0x3b4);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_NULLSRC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_NULLSRC: MessageId = MessageId(0x3b5);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALIDSRCRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_INVALIDSRCRESOURCE: MessageId = MessageId(0x3b6);
/// The header's `D3D12_MESSAGE_ID_PIPELINE_STATE_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PIPELINE_STATE_TYPE_MISMATCH: MessageId = MessageId(0x3b7);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DISPATCH_ROOT_SIGNATURE_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DISPATCH_ROOT_SIGNATURE_NOT_SET: MessageId =
    MessageId(0x3b8);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DISPATCH_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DISPATCH_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x3b9);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_ZERO_BARRIERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_ZERO_BARRIERS: MessageId = MessageId(0x3ba);
/// The header's `D3D12_MESSAGE_ID_BEGIN_END_EVENT_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_BEGIN_END_EVENT_MISMATCH: MessageId = MessageId(0x3bb);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_POSSIBLE_BEFORE_AFTER_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_POSSIBLE_BEFORE_AFTER_MISMATCH: MessageId =
    MessageId(0x3bc);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_MISMATCHING_BEGIN_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_MISMATCHING_BEGIN_END: MessageId = MessageId(0x3bd);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_INVALID_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_INVALID_RESOURCE: MessageId = MessageId(0x3be);
/// The header's `D3D12_MESSAGE_ID_USE_OF_ZERO_REFCOUNT_OBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_USE_OF_ZERO_REFCOUNT_OBJECT: MessageId = MessageId(0x3bf);
/// The header's `D3D12_MESSAGE_ID_OBJECT_EVICTED_WHILE_STILL_IN_USE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OBJECT_EVICTED_WHILE_STILL_IN_USE: MessageId = MessageId(0x3c0);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_ROOT_DESCRIPTOR_ACCESS_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_ROOT_DESCRIPTOR_ACCESS_OUT_OF_BOUNDS: MessageId =
    MessageId(0x3c1);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINELIBRARY_INVALIDLIBRARYBLOB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINELIBRARY_INVALIDLIBRARYBLOB: MessageId = MessageId(0x3c2);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINELIBRARY_DRIVERVERSIONMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINELIBRARY_DRIVERVERSIONMISMATCH: MessageId =
    MessageId(0x3c3);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINELIBRARY_ADAPTERVERSIONMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINELIBRARY_ADAPTERVERSIONMISMATCH: MessageId =
    MessageId(0x3c4);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINELIBRARY_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINELIBRARY_UNSUPPORTED: MessageId = MessageId(0x3c5);
/// The header's `D3D12_MESSAGE_ID_CREATE_PIPELINELIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_PIPELINELIBRARY: MessageId = MessageId(0x3c6);
/// The header's `D3D12_MESSAGE_ID_LIVE_PIPELINELIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_PIPELINELIBRARY: MessageId = MessageId(0x3c7);
/// The header's `D3D12_MESSAGE_ID_DESTROY_PIPELINELIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_PIPELINELIBRARY: MessageId = MessageId(0x3c8);
/// The header's `D3D12_MESSAGE_ID_STOREPIPELINE_NONAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_STOREPIPELINE_NONAME: MessageId = MessageId(0x3c9);
/// The header's `D3D12_MESSAGE_ID_STOREPIPELINE_DUPLICATENAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_STOREPIPELINE_DUPLICATENAME: MessageId = MessageId(0x3ca);
/// The header's `D3D12_MESSAGE_ID_LOADPIPELINE_NAMENOTFOUND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LOADPIPELINE_NAMENOTFOUND: MessageId = MessageId(0x3cb);
/// The header's `D3D12_MESSAGE_ID_LOADPIPELINE_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LOADPIPELINE_INVALIDDESC: MessageId = MessageId(0x3cc);
/// The header's `D3D12_MESSAGE_ID_PIPELINELIBRARY_SERIALIZE_NOTENOUGHMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PIPELINELIBRARY_SERIALIZE_NOTENOUGHMEMORY: MessageId = MessageId(0x3cd);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_PS_OUTPUT_RT_OUTPUT_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_PS_OUTPUT_RT_OUTPUT_MISMATCH: MessageId =
    MessageId(0x3ce);
/// The header's `D3D12_MESSAGE_ID_SETEVENTONMULTIPLEFENCECOMPLETION_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETEVENTONMULTIPLEFENCECOMPLETION_INVALIDFLAGS: MessageId =
    MessageId(0x3cf);
/// The header's `D3D12_MESSAGE_ID_CREATE_QUEUE_VIDEO_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_QUEUE_VIDEO_NOT_SUPPORTED: MessageId = MessageId(0x3d0);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_ALLOCATOR_VIDEO_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_ALLOCATOR_VIDEO_NOT_SUPPORTED: MessageId =
    MessageId(0x3d1);
/// The header's `D3D12_MESSAGE_ID_CREATEQUERY_HEAP_VIDEO_DECODE_STATISTICS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEQUERY_HEAP_VIDEO_DECODE_STATISTICS_NOT_SUPPORTED: MessageId =
    MessageId(0x3d2);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEODECODECOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEODECODECOMMANDLIST: MessageId = MessageId(0x3d3);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEODECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEODECODER: MessageId = MessageId(0x3d4);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEODECODESTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEODECODESTREAM: MessageId = MessageId(0x3d5);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEODECODECOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEODECODECOMMANDLIST: MessageId = MessageId(0x3d6);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEODECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEODECODER: MessageId = MessageId(0x3d7);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEODECODESTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEODECODESTREAM: MessageId = MessageId(0x3d8);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEODECODECOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEODECODECOMMANDLIST: MessageId = MessageId(0x3d9);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEODECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEODECODER: MessageId = MessageId(0x3da);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEODECODESTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEODECODESTREAM: MessageId = MessageId(0x3db);
/// The header's `D3D12_MESSAGE_ID_DECODE_FRAME_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DECODE_FRAME_INVALID_PARAMETERS: MessageId = MessageId(0x3dc);
/// The header's `D3D12_MESSAGE_ID_DEPRECATED_API`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEPRECATED_API: MessageId = MessageId(0x3dd);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_MISMATCHING_COMMAND_LIST_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_MISMATCHING_COMMAND_LIST_TYPE: MessageId =
    MessageId(0x3de);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DESCRIPTOR_TABLE_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DESCRIPTOR_TABLE_NOT_SET: MessageId = MessageId(0x3df);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_ROOT_CONSTANT_BUFFER_VIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_ROOT_CONSTANT_BUFFER_VIEW_NOT_SET: MessageId =
    MessageId(0x3e0);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_ROOT_SHADER_RESOURCE_VIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_ROOT_SHADER_RESOURCE_VIEW_NOT_SET: MessageId =
    MessageId(0x3e1);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_ROOT_UNORDERED_ACCESS_VIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_ROOT_UNORDERED_ACCESS_VIEW_NOT_SET: MessageId =
    MessageId(0x3e2);
/// The header's `D3D12_MESSAGE_ID_DISCARD_INVALID_SUBRESOURCE_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DISCARD_INVALID_SUBRESOURCE_RANGE: MessageId = MessageId(0x3e3);
/// The header's `D3D12_MESSAGE_ID_DISCARD_ONE_SUBRESOURCE_FOR_MIPS_WITH_RECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DISCARD_ONE_SUBRESOURCE_FOR_MIPS_WITH_RECTS: MessageId =
    MessageId(0x3e4);
/// The header's `D3D12_MESSAGE_ID_DISCARD_NO_RECTS_FOR_NON_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DISCARD_NO_RECTS_FOR_NON_TEXTURE2D: MessageId = MessageId(0x3e5);
/// The header's `D3D12_MESSAGE_ID_COPY_ON_SAME_SUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPY_ON_SAME_SUBRESOURCE: MessageId = MessageId(0x3e6);
/// The header's `D3D12_MESSAGE_ID_SETRESIDENCYPRIORITY_INVALID_PAGEABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETRESIDENCYPRIORITY_INVALID_PAGEABLE: MessageId = MessageId(0x3e7);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_UNSUPPORTED: MessageId = MessageId(0x3e8);
/// The header's `D3D12_MESSAGE_ID_STATIC_DESCRIPTOR_INVALID_DESCRIPTOR_CHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_STATIC_DESCRIPTOR_INVALID_DESCRIPTOR_CHANGE: MessageId =
    MessageId(0x3e9);
/// The header's `D3D12_MESSAGE_ID_DATA_STATIC_DESCRIPTOR_INVALID_DATA_CHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DATA_STATIC_DESCRIPTOR_INVALID_DATA_CHANGE: MessageId = MessageId(0x3ea);
/// The header's `D3D12_MESSAGE_ID_DATA_STATIC_WHILE_SET_AT_EXECUTE_DESCRIPTOR_INVALID_DATA_CHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DATA_STATIC_WHILE_SET_AT_EXECUTE_DESCRIPTOR_INVALID_DATA_CHANGE:
    MessageId = MessageId(0x3eb);
/// The header's `D3D12_MESSAGE_ID_EXECUTE_BUNDLE_STATIC_DESCRIPTOR_DATA_STATIC_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTE_BUNDLE_STATIC_DESCRIPTOR_DATA_STATIC_NOT_SET: MessageId =
    MessageId(0x3ec);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_RESOURCE_ACCESS_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_RESOURCE_ACCESS_OUT_OF_BOUNDS: MessageId =
    MessageId(0x3ed);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_SAMPLER_MODE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_SAMPLER_MODE_MISMATCH: MessageId = MessageId(0x3ee);
/// The header's `D3D12_MESSAGE_ID_CREATE_FENCE_INVALID_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_FENCE_INVALID_FLAGS: MessageId = MessageId(0x3ef);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_BARRIER_DUPLICATE_SUBRESOURCE_TRANSITIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_BARRIER_DUPLICATE_SUBRESOURCE_TRANSITIONS: MessageId =
    MessageId(0x3f0);
/// The header's `D3D12_MESSAGE_ID_SETRESIDENCYPRIORITY_INVALID_PRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETRESIDENCYPRIORITY_INVALID_PRIORITY: MessageId = MessageId(0x3f1);
/// The header's `D3D12_MESSAGE_ID_CREATE_DESCRIPTOR_HEAP_LARGE_NUM_DESCRIPTORS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_DESCRIPTOR_HEAP_LARGE_NUM_DESCRIPTORS: MessageId =
    MessageId(0x3f5);
/// The header's `D3D12_MESSAGE_ID_BEGIN_EVENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_BEGIN_EVENT: MessageId = MessageId(0x3f6);
/// The header's `D3D12_MESSAGE_ID_END_EVENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_END_EVENT: MessageId = MessageId(0x3f7);
/// The header's `D3D12_MESSAGE_ID_CREATEDEVICE_DEBUG_LAYER_STARTUP_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEVICE_DEBUG_LAYER_STARTUP_OPTIONS: MessageId = MessageId(0x3f8);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_DEPTHBOUNDSTEST_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_DEPTHBOUNDSTEST_UNSUPPORTED: MessageId =
    MessageId(0x3f9);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_DUPLICATE_SUBOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_DUPLICATE_SUBOBJECT: MessageId = MessageId(0x3fa);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_UNKNOWN_SUBOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_UNKNOWN_SUBOBJECT: MessageId = MessageId(0x3fb);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_ZERO_SIZE_STREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_ZERO_SIZE_STREAM: MessageId = MessageId(0x3fc);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_INVALID_STREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_INVALID_STREAM: MessageId = MessageId(0x3fd);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CANNOT_DEDUCE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_CANNOT_DEDUCE_TYPE: MessageId = MessageId(0x3fe);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_STATIC_DESCRIPTOR_RESOURCE_DIMENSION_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_STATIC_DESCRIPTOR_RESOURCE_DIMENSION_MISMATCH: MessageId =
    MessageId(0x3ff);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_QUEUE_INSUFFICIENT_PRIVILEGE_FOR_GLOBAL_REALTIME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_QUEUE_INSUFFICIENT_PRIVILEGE_FOR_GLOBAL_REALTIME:
    MessageId = MessageId(0x400);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_QUEUE_INSUFFICIENT_HARDWARE_SUPPORT_FOR_GLOBAL_REALTIME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_QUEUE_INSUFFICIENT_HARDWARE_SUPPORT_FOR_GLOBAL_REALTIME: MessageId = MessageId(0x401);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_ARCHITECTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_ARCHITECTURE: MessageId = MessageId(0x402);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_NULL_DST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_NULL_DST: MessageId = MessageId(0x403);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_DST_RESOURCE_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_DST_RESOURCE_DIMENSION: MessageId =
    MessageId(0x404);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_DST_RANGE_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_DST_RANGE_OUT_OF_BOUNDS: MessageId = MessageId(0x405);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_NULL_SRC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_NULL_SRC: MessageId = MessageId(0x406);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_SRC_RESOURCE_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_SRC_RESOURCE_DIMENSION: MessageId =
    MessageId(0x407);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_SRC_RANGE_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_SRC_RANGE_OUT_OF_BOUNDS: MessageId = MessageId(0x408);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_OFFSET_ALIGNMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_OFFSET_ALIGNMENT: MessageId = MessageId(0x409);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_NULL_DEPENDENT_RESOURCES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_NULL_DEPENDENT_RESOURCES: MessageId = MessageId(0x40a);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_NULL_DEPENDENT_SUBRESOURCE_RANGES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_NULL_DEPENDENT_SUBRESOURCE_RANGES: MessageId =
    MessageId(0x40b);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_DEPENDENT_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_DEPENDENT_RESOURCE: MessageId =
    MessageId(0x40c);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_DEPENDENT_SUBRESOURCE_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_DEPENDENT_SUBRESOURCE_RANGE: MessageId =
    MessageId(0x40d);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_DEPENDENT_SUBRESOURCE_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_DEPENDENT_SUBRESOURCE_OUT_OF_BOUNDS: MessageId =
    MessageId(0x40e);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_DEPENDENT_RANGE_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_DEPENDENT_RANGE_OUT_OF_BOUNDS: MessageId =
    MessageId(0x40f);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_ZERO_DEPENDENCIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_ZERO_DEPENDENCIES: MessageId = MessageId(0x410);
/// The header's `D3D12_MESSAGE_ID_DEVICE_CREATE_SHARED_HANDLE_INVALIDARG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_CREATE_SHARED_HANDLE_INVALIDARG: MessageId = MessageId(0x411);
/// The header's `D3D12_MESSAGE_ID_DESCRIPTOR_HANDLE_WITH_INVALID_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESCRIPTOR_HANDLE_WITH_INVALID_RESOURCE: MessageId = MessageId(0x412);
/// The header's `D3D12_MESSAGE_ID_SETDEPTHBOUNDS_INVALIDARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETDEPTHBOUNDS_INVALIDARGS: MessageId = MessageId(0x413);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_RESOURCE_STATE_IMPRECISE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_RESOURCE_STATE_IMPRECISE: MessageId =
    MessageId(0x414);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_PIPELINE_STATE_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_PIPELINE_STATE_NOT_SET: MessageId = MessageId(0x415);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_MODEL_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_SHADER_MODEL_MISMATCH: MessageId =
    MessageId(0x416);
/// The header's `D3D12_MESSAGE_ID_OBJECT_ACCESSED_WHILE_STILL_IN_USE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OBJECT_ACCESSED_WHILE_STILL_IN_USE: MessageId = MessageId(0x417);
/// The header's `D3D12_MESSAGE_ID_PROGRAMMABLE_MSAA_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PROGRAMMABLE_MSAA_UNSUPPORTED: MessageId = MessageId(0x418);
/// The header's `D3D12_MESSAGE_ID_SETSAMPLEPOSITIONS_INVALIDARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETSAMPLEPOSITIONS_INVALIDARGS: MessageId = MessageId(0x419);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCEREGION_INVALID_RECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCEREGION_INVALID_RECT: MessageId = MessageId(0x41a);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEODECODECOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEODECODECOMMANDQUEUE: MessageId = MessageId(0x41b);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOPROCESSCOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOPROCESSCOMMANDLIST: MessageId = MessageId(0x41c);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOPROCESSCOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOPROCESSCOMMANDQUEUE: MessageId = MessageId(0x41d);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEODECODECOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEODECODECOMMANDQUEUE: MessageId = MessageId(0x41e);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOPROCESSCOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOPROCESSCOMMANDLIST: MessageId = MessageId(0x41f);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOPROCESSCOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOPROCESSCOMMANDQUEUE: MessageId = MessageId(0x420);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEODECODECOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEODECODECOMMANDQUEUE: MessageId = MessageId(0x421);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOPROCESSCOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOPROCESSCOMMANDLIST: MessageId = MessageId(0x422);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOPROCESSCOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOPROCESSCOMMANDQUEUE: MessageId = MessageId(0x423);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOPROCESSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOPROCESSOR: MessageId = MessageId(0x424);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOPROCESSSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOPROCESSSTREAM: MessageId = MessageId(0x425);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOPROCESSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOPROCESSOR: MessageId = MessageId(0x426);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOPROCESSSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOPROCESSSTREAM: MessageId = MessageId(0x427);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOPROCESSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOPROCESSOR: MessageId = MessageId(0x428);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOPROCESSSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOPROCESSSTREAM: MessageId = MessageId(0x429);
/// The header's `D3D12_MESSAGE_ID_PROCESS_FRAME_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PROCESS_FRAME_INVALID_PARAMETERS: MessageId = MessageId(0x42a);
/// The header's `D3D12_MESSAGE_ID_COPY_INVALIDLAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPY_INVALIDLAYOUT: MessageId = MessageId(0x42b);
/// The header's `D3D12_MESSAGE_ID_CREATE_CRYPTO_SESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_CRYPTO_SESSION: MessageId = MessageId(0x42c);
/// The header's `D3D12_MESSAGE_ID_CREATE_CRYPTO_SESSION_POLICY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_CRYPTO_SESSION_POLICY: MessageId = MessageId(0x42d);
/// The header's `D3D12_MESSAGE_ID_CREATE_PROTECTED_RESOURCE_SESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_PROTECTED_RESOURCE_SESSION: MessageId = MessageId(0x42e);
/// The header's `D3D12_MESSAGE_ID_LIVE_CRYPTO_SESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_CRYPTO_SESSION: MessageId = MessageId(0x42f);
/// The header's `D3D12_MESSAGE_ID_LIVE_CRYPTO_SESSION_POLICY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_CRYPTO_SESSION_POLICY: MessageId = MessageId(0x430);
/// The header's `D3D12_MESSAGE_ID_LIVE_PROTECTED_RESOURCE_SESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_PROTECTED_RESOURCE_SESSION: MessageId = MessageId(0x431);
/// The header's `D3D12_MESSAGE_ID_DESTROY_CRYPTO_SESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_CRYPTO_SESSION: MessageId = MessageId(0x432);
/// The header's `D3D12_MESSAGE_ID_DESTROY_CRYPTO_SESSION_POLICY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_CRYPTO_SESSION_POLICY: MessageId = MessageId(0x433);
/// The header's `D3D12_MESSAGE_ID_DESTROY_PROTECTED_RESOURCE_SESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_PROTECTED_RESOURCE_SESSION: MessageId = MessageId(0x434);
/// The header's `D3D12_MESSAGE_ID_PROTECTED_RESOURCE_SESSION_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PROTECTED_RESOURCE_SESSION_UNSUPPORTED: MessageId = MessageId(0x435);
/// The header's `D3D12_MESSAGE_ID_FENCE_INVALIDOPERATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_FENCE_INVALIDOPERATION: MessageId = MessageId(0x436);
/// The header's `D3D12_MESSAGE_ID_CREATEQUERY_HEAP_COPY_QUEUE_TIMESTAMPS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEQUERY_HEAP_COPY_QUEUE_TIMESTAMPS_NOT_SUPPORTED: MessageId =
    MessageId(0x437);
/// The header's `D3D12_MESSAGE_ID_SAMPLEPOSITIONS_MISMATCH_DEFERRED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLEPOSITIONS_MISMATCH_DEFERRED: MessageId = MessageId(0x438);
/// The header's `D3D12_MESSAGE_ID_SAMPLEPOSITIONS_MISMATCH_RECORDTIME_ASSUMEDFROMFIRSTUSE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLEPOSITIONS_MISMATCH_RECORDTIME_ASSUMEDFROMFIRSTUSE: MessageId =
    MessageId(0x439);
/// The header's `D3D12_MESSAGE_ID_SAMPLEPOSITIONS_MISMATCH_RECORDTIME_ASSUMEDFROMCLEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLEPOSITIONS_MISMATCH_RECORDTIME_ASSUMEDFROMCLEAR: MessageId =
    MessageId(0x43a);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEODECODERHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEODECODERHEAP: MessageId = MessageId(0x43b);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEODECODERHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEODECODERHEAP: MessageId = MessageId(0x43c);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEODECODERHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEODECODERHEAP: MessageId = MessageId(0x43d);
/// The header's `D3D12_MESSAGE_ID_OPENEXISTINGHEAP_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OPENEXISTINGHEAP_INVALIDARG_RETURN: MessageId = MessageId(0x43e);
/// The header's `D3D12_MESSAGE_ID_OPENEXISTINGHEAP_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OPENEXISTINGHEAP_OUTOFMEMORY_RETURN: MessageId = MessageId(0x43f);
/// The header's `D3D12_MESSAGE_ID_OPENEXISTINGHEAP_INVALIDADDRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OPENEXISTINGHEAP_INVALIDADDRESS: MessageId = MessageId(0x440);
/// The header's `D3D12_MESSAGE_ID_OPENEXISTINGHEAP_INVALIDHANDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OPENEXISTINGHEAP_INVALIDHANDLE: MessageId = MessageId(0x441);
/// The header's `D3D12_MESSAGE_ID_WRITEBUFFERIMMEDIATE_INVALID_DEST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITEBUFFERIMMEDIATE_INVALID_DEST: MessageId = MessageId(0x442);
/// The header's `D3D12_MESSAGE_ID_WRITEBUFFERIMMEDIATE_INVALID_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITEBUFFERIMMEDIATE_INVALID_MODE: MessageId = MessageId(0x443);
/// The header's `D3D12_MESSAGE_ID_WRITEBUFFERIMMEDIATE_INVALID_ALIGNMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITEBUFFERIMMEDIATE_INVALID_ALIGNMENT: MessageId = MessageId(0x444);
/// The header's `D3D12_MESSAGE_ID_WRITEBUFFERIMMEDIATE_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITEBUFFERIMMEDIATE_NOT_SUPPORTED: MessageId = MessageId(0x445);
/// The header's `D3D12_MESSAGE_ID_SETVIEWINSTANCEMASK_INVALIDARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETVIEWINSTANCEMASK_INVALIDARGS: MessageId = MessageId(0x446);
/// The header's `D3D12_MESSAGE_ID_VIEW_INSTANCING_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIEW_INSTANCING_UNSUPPORTED: MessageId = MessageId(0x447);
/// The header's `D3D12_MESSAGE_ID_VIEW_INSTANCING_INVALIDARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIEW_INSTANCING_INVALIDARGS: MessageId = MessageId(0x448);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_MISMATCH_DECODE_REFERENCE_ONLY_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_MISMATCH_DECODE_REFERENCE_ONLY_FLAG: MessageId =
    MessageId(0x449);
/// The header's `D3D12_MESSAGE_ID_COPYRESOURCE_MISMATCH_DECODE_REFERENCE_ONLY_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYRESOURCE_MISMATCH_DECODE_REFERENCE_ONLY_FLAG: MessageId =
    MessageId(0x44a);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEO_DECODE_HEAP_CAPS_FAILURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEO_DECODE_HEAP_CAPS_FAILURE: MessageId = MessageId(0x44b);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEO_DECODE_HEAP_CAPS_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEO_DECODE_HEAP_CAPS_UNSUPPORTED: MessageId = MessageId(0x44c);
/// The header's `D3D12_MESSAGE_ID_VIDEO_DECODE_SUPPORT_INVALID_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_DECODE_SUPPORT_INVALID_INPUT: MessageId = MessageId(0x44d);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEO_DECODER_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEO_DECODER_UNSUPPORTED: MessageId = MessageId(0x44e);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_METADATA_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_METADATA_ERROR: MessageId = MessageId(0x44f);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_VIEW_INSTANCING_VERTEX_SIZE_EXCEEDED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_VIEW_INSTANCING_VERTEX_SIZE_EXCEEDED:
    MessageId = MessageId(0x450);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_RUNTIME_INTERNAL_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_RUNTIME_INTERNAL_ERROR: MessageId =
    MessageId(0x451);
/// The header's `D3D12_MESSAGE_ID_NO_VIDEO_API_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_NO_VIDEO_API_SUPPORT: MessageId = MessageId(0x452);
/// The header's `D3D12_MESSAGE_ID_VIDEO_PROCESS_SUPPORT_INVALID_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_PROCESS_SUPPORT_INVALID_INPUT: MessageId = MessageId(0x453);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEO_PROCESSOR_CAPS_FAILURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEO_PROCESSOR_CAPS_FAILURE: MessageId = MessageId(0x454);
/// The header's `D3D12_MESSAGE_ID_VIDEO_PROCESS_SUPPORT_UNSUPPORTED_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_PROCESS_SUPPORT_UNSUPPORTED_FORMAT: MessageId = MessageId(0x455);
/// The header's `D3D12_MESSAGE_ID_VIDEO_DECODE_FRAME_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_DECODE_FRAME_INVALID_ARGUMENT: MessageId = MessageId(0x456);
/// The header's `D3D12_MESSAGE_ID_ENQUEUE_MAKE_RESIDENT_INVALID_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ENQUEUE_MAKE_RESIDENT_INVALID_FLAGS: MessageId = MessageId(0x457);
/// The header's `D3D12_MESSAGE_ID_OPENEXISTINGHEAP_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OPENEXISTINGHEAP_UNSUPPORTED: MessageId = MessageId(0x458);
/// The header's `D3D12_MESSAGE_ID_VIDEO_PROCESS_FRAMES_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_PROCESS_FRAMES_INVALID_ARGUMENT: MessageId = MessageId(0x459);
/// The header's `D3D12_MESSAGE_ID_VIDEO_DECODE_SUPPORT_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_DECODE_SUPPORT_UNSUPPORTED: MessageId = MessageId(0x45a);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMANDRECORDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMANDRECORDER: MessageId = MessageId(0x45b);
/// The header's `D3D12_MESSAGE_ID_LIVE_COMMANDRECORDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_COMMANDRECORDER: MessageId = MessageId(0x45c);
/// The header's `D3D12_MESSAGE_ID_DESTROY_COMMANDRECORDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_COMMANDRECORDER: MessageId = MessageId(0x45d);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_RECORDER_VIDEO_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_RECORDER_VIDEO_NOT_SUPPORTED: MessageId =
    MessageId(0x45e);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_RECORDER_INVALID_SUPPORT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_RECORDER_INVALID_SUPPORT_FLAGS: MessageId =
    MessageId(0x45f);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_RECORDER_INVALID_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_RECORDER_INVALID_FLAGS: MessageId = MessageId(0x460);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_RECORDER_MORE_RECORDERS_THAN_LOGICAL_PROCESSORS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_RECORDER_MORE_RECORDERS_THAN_LOGICAL_PROCESSORS:
    MessageId = MessageId(0x461);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMANDPOOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMANDPOOL: MessageId = MessageId(0x462);
/// The header's `D3D12_MESSAGE_ID_LIVE_COMMANDPOOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_COMMANDPOOL: MessageId = MessageId(0x463);
/// The header's `D3D12_MESSAGE_ID_DESTROY_COMMANDPOOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_COMMANDPOOL: MessageId = MessageId(0x464);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_POOL_INVALID_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_POOL_INVALID_FLAGS: MessageId = MessageId(0x465);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_LIST_VIDEO_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_LIST_VIDEO_NOT_SUPPORTED: MessageId = MessageId(0x466);
/// The header's `D3D12_MESSAGE_ID_COMMAND_RECORDER_SUPPORT_FLAGS_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_RECORDER_SUPPORT_FLAGS_MISMATCH: MessageId = MessageId(0x467);
/// The header's `D3D12_MESSAGE_ID_COMMAND_RECORDER_CONTENTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_RECORDER_CONTENTION: MessageId = MessageId(0x468);
/// The header's `D3D12_MESSAGE_ID_COMMAND_RECORDER_USAGE_WITH_CREATECOMMANDLIST_COMMAND_LIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_RECORDER_USAGE_WITH_CREATECOMMANDLIST_COMMAND_LIST: MessageId =
    MessageId(0x469);
/// The header's `D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_USAGE_WITH_CREATECOMMANDLIST1_COMMAND_LIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_ALLOCATOR_USAGE_WITH_CREATECOMMANDLIST1_COMMAND_LIST: MessageId =
    MessageId(0x46a);
/// The header's `D3D12_MESSAGE_ID_CANNOT_EXECUTE_EMPTY_COMMAND_LIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CANNOT_EXECUTE_EMPTY_COMMAND_LIST: MessageId = MessageId(0x46b);
/// The header's `D3D12_MESSAGE_ID_CANNOT_RESET_COMMAND_POOL_WITH_OPEN_COMMAND_LISTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CANNOT_RESET_COMMAND_POOL_WITH_OPEN_COMMAND_LISTS: MessageId =
    MessageId(0x46c);
/// The header's `D3D12_MESSAGE_ID_CANNOT_USE_COMMAND_RECORDER_WITHOUT_CURRENT_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CANNOT_USE_COMMAND_RECORDER_WITHOUT_CURRENT_TARGET: MessageId =
    MessageId(0x46d);
/// The header's `D3D12_MESSAGE_ID_CANNOT_CHANGE_COMMAND_RECORDER_TARGET_WHILE_RECORDING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CANNOT_CHANGE_COMMAND_RECORDER_TARGET_WHILE_RECORDING: MessageId =
    MessageId(0x46e);
/// The header's `D3D12_MESSAGE_ID_COMMAND_POOL_SYNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_POOL_SYNC: MessageId = MessageId(0x46f);
/// The header's `D3D12_MESSAGE_ID_EVICT_UNDERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EVICT_UNDERFLOW: MessageId = MessageId(0x470);
/// The header's `D3D12_MESSAGE_ID_CREATE_META_COMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_META_COMMAND: MessageId = MessageId(0x471);
/// The header's `D3D12_MESSAGE_ID_LIVE_META_COMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_META_COMMAND: MessageId = MessageId(0x472);
/// The header's `D3D12_MESSAGE_ID_DESTROY_META_COMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_META_COMMAND: MessageId = MessageId(0x473);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALID_DST_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALID_DST_RESOURCE: MessageId = MessageId(0x474);
/// The header's `D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALID_SRC_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYBUFFERREGION_INVALID_SRC_RESOURCE: MessageId = MessageId(0x475);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_DST_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_DST_RESOURCE: MessageId = MessageId(0x476);
/// The header's `D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_SRC_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ATOMICCOPYBUFFER_INVALID_SRC_RESOURCE: MessageId = MessageId(0x477);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_NULL_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_NULL_BUFFER: MessageId = MessageId(0x478);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_NULL_RESOURCE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_NULL_RESOURCE_DESC: MessageId =
    MessageId(0x479);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_UNSUPPORTED: MessageId = MessageId(0x47a);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_BUFFER_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_BUFFER_DIMENSION: MessageId =
    MessageId(0x47b);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_BUFFER_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_BUFFER_FLAGS: MessageId =
    MessageId(0x47c);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_BUFFER_OFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_BUFFER_OFFSET: MessageId =
    MessageId(0x47d);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_RESOURCE_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_RESOURCE_DIMENSION: MessageId =
    MessageId(0x47e);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_RESOURCE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_INVALID_RESOURCE_FLAGS: MessageId =
    MessageId(0x47f);
/// The header's `D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPLACEDRESOURCEONBUFFER_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x480);
/// The header's `D3D12_MESSAGE_ID_CANNOT_CREATE_GRAPHICS_AND_VIDEO_COMMAND_RECORDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CANNOT_CREATE_GRAPHICS_AND_VIDEO_COMMAND_RECORDER: MessageId =
    MessageId(0x481);
/// The header's `D3D12_MESSAGE_ID_UPDATETILEMAPPINGS_POSSIBLY_MISMATCHING_PROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UPDATETILEMAPPINGS_POSSIBLY_MISMATCHING_PROPERTIES: MessageId =
    MessageId(0x482);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_LIST_INVALID_COMMAND_LIST_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_LIST_INVALID_COMMAND_LIST_TYPE: MessageId =
    MessageId(0x483);
/// The header's `D3D12_MESSAGE_ID_CLEARUNORDEREDACCESSVIEW_INCOMPATIBLE_WITH_STRUCTURED_BUFFERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CLEARUNORDEREDACCESSVIEW_INCOMPATIBLE_WITH_STRUCTURED_BUFFERS:
    MessageId = MessageId(0x484);
/// The header's `D3D12_MESSAGE_ID_COMPUTE_ONLY_DEVICE_OPERATION_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMPUTE_ONLY_DEVICE_OPERATION_UNSUPPORTED: MessageId = MessageId(0x485);
/// The header's `D3D12_MESSAGE_ID_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INVALID: MessageId =
    MessageId(0x486);
/// The header's `D3D12_MESSAGE_ID_EMIT_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EMIT_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_INVALID:
    MessageId = MessageId(0x487);
/// The header's `D3D12_MESSAGE_ID_COPY_RAYTRACING_ACCELERATION_STRUCTURE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPY_RAYTRACING_ACCELERATION_STRUCTURE_INVALID: MessageId =
    MessageId(0x488);
/// The header's `D3D12_MESSAGE_ID_DISPATCH_RAYS_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DISPATCH_RAYS_INVALID: MessageId = MessageId(0x489);
/// The header's `D3D12_MESSAGE_ID_GET_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GET_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO_INVALID: MessageId =
    MessageId(0x48a);
/// The header's `D3D12_MESSAGE_ID_CREATE_LIFETIMETRACKER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_LIFETIMETRACKER: MessageId = MessageId(0x48b);
/// The header's `D3D12_MESSAGE_ID_LIVE_LIFETIMETRACKER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_LIFETIMETRACKER: MessageId = MessageId(0x48c);
/// The header's `D3D12_MESSAGE_ID_DESTROY_LIFETIMETRACKER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_LIFETIMETRACKER: MessageId = MessageId(0x48d);
/// The header's `D3D12_MESSAGE_ID_DESTROYOWNEDOBJECT_OBJECTNOTOWNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROYOWNEDOBJECT_OBJECTNOTOWNED: MessageId = MessageId(0x48e);
/// The header's `D3D12_MESSAGE_ID_CREATE_TRACKEDWORKLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_TRACKEDWORKLOAD: MessageId = MessageId(0x48f);
/// The header's `D3D12_MESSAGE_ID_LIVE_TRACKEDWORKLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_TRACKEDWORKLOAD: MessageId = MessageId(0x490);
/// The header's `D3D12_MESSAGE_ID_DESTROY_TRACKEDWORKLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_TRACKEDWORKLOAD: MessageId = MessageId(0x491);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_ERROR: MessageId = MessageId(0x492);
/// The header's `D3D12_MESSAGE_ID_META_COMMAND_ID_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_META_COMMAND_ID_INVALID: MessageId = MessageId(0x493);
/// The header's `D3D12_MESSAGE_ID_META_COMMAND_UNSUPPORTED_PARAMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_META_COMMAND_UNSUPPORTED_PARAMS: MessageId = MessageId(0x494);
/// The header's `D3D12_MESSAGE_ID_META_COMMAND_FAILED_ENUMERATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_META_COMMAND_FAILED_ENUMERATION: MessageId = MessageId(0x495);
/// The header's `D3D12_MESSAGE_ID_META_COMMAND_PARAMETER_SIZE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_META_COMMAND_PARAMETER_SIZE_MISMATCH: MessageId = MessageId(0x496);
/// The header's `D3D12_MESSAGE_ID_UNINITIALIZED_META_COMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNINITIALIZED_META_COMMAND: MessageId = MessageId(0x497);
/// The header's `D3D12_MESSAGE_ID_META_COMMAND_INVALID_GPU_VIRTUAL_ADDRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_META_COMMAND_INVALID_GPU_VIRTUAL_ADDRESS: MessageId = MessageId(0x498);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOENCODECOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOENCODECOMMANDLIST: MessageId = MessageId(0x499);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOENCODECOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOENCODECOMMANDLIST: MessageId = MessageId(0x49a);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOENCODECOMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOENCODECOMMANDLIST: MessageId = MessageId(0x49b);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOENCODECOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOENCODECOMMANDQUEUE: MessageId = MessageId(0x49c);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOENCODECOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOENCODECOMMANDQUEUE: MessageId = MessageId(0x49d);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOENCODECOMMANDQUEUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOENCODECOMMANDQUEUE: MessageId = MessageId(0x49e);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOMOTIONESTIMATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOMOTIONESTIMATOR: MessageId = MessageId(0x49f);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOMOTIONESTIMATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOMOTIONESTIMATOR: MessageId = MessageId(0x4a0);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOMOTIONESTIMATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOMOTIONESTIMATOR: MessageId = MessageId(0x4a1);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOMOTIONVECTORHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOMOTIONVECTORHEAP: MessageId = MessageId(0x4a2);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOMOTIONVECTORHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOMOTIONVECTORHEAP: MessageId = MessageId(0x4a3);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOMOTIONVECTORHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOMOTIONVECTORHEAP: MessageId = MessageId(0x4a4);
/// The header's `D3D12_MESSAGE_ID_MULTIPLE_TRACKED_WORKLOADS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MULTIPLE_TRACKED_WORKLOADS: MessageId = MessageId(0x4a5);
/// The header's `D3D12_MESSAGE_ID_MULTIPLE_TRACKED_WORKLOAD_PAIRS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MULTIPLE_TRACKED_WORKLOAD_PAIRS: MessageId = MessageId(0x4a6);
/// The header's `D3D12_MESSAGE_ID_OUT_OF_ORDER_TRACKED_WORKLOAD_PAIR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OUT_OF_ORDER_TRACKED_WORKLOAD_PAIR: MessageId = MessageId(0x4a7);
/// The header's `D3D12_MESSAGE_ID_CANNOT_ADD_TRACKED_WORKLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CANNOT_ADD_TRACKED_WORKLOAD: MessageId = MessageId(0x4a8);
/// The header's `D3D12_MESSAGE_ID_INCOMPLETE_TRACKED_WORKLOAD_PAIR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INCOMPLETE_TRACKED_WORKLOAD_PAIR: MessageId = MessageId(0x4a9);
/// The header's `D3D12_MESSAGE_ID_CREATE_STATE_OBJECT_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_STATE_OBJECT_ERROR: MessageId = MessageId(0x4aa);
/// The header's `D3D12_MESSAGE_ID_GET_SHADER_IDENTIFIER_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GET_SHADER_IDENTIFIER_ERROR: MessageId = MessageId(0x4ab);
/// The header's `D3D12_MESSAGE_ID_GET_SHADER_STACK_SIZE_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GET_SHADER_STACK_SIZE_ERROR: MessageId = MessageId(0x4ac);
/// The header's `D3D12_MESSAGE_ID_GET_PIPELINE_STACK_SIZE_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GET_PIPELINE_STACK_SIZE_ERROR: MessageId = MessageId(0x4ad);
/// The header's `D3D12_MESSAGE_ID_SET_PIPELINE_STACK_SIZE_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_PIPELINE_STACK_SIZE_ERROR: MessageId = MessageId(0x4ae);
/// The header's `D3D12_MESSAGE_ID_GET_SHADER_IDENTIFIER_SIZE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GET_SHADER_IDENTIFIER_SIZE_INVALID: MessageId = MessageId(0x4af);
/// The header's `D3D12_MESSAGE_ID_CHECK_DRIVER_MATCHING_IDENTIFIER_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CHECK_DRIVER_MATCHING_IDENTIFIER_INVALID: MessageId = MessageId(0x4b0);
/// The header's `D3D12_MESSAGE_ID_CHECK_DRIVER_MATCHING_IDENTIFIER_DRIVER_REPORTED_ISSUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CHECK_DRIVER_MATCHING_IDENTIFIER_DRIVER_REPORTED_ISSUE: MessageId =
    MessageId(0x4b1);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_INVALID_RESOURCE_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_INVALID_RESOURCE_BARRIER: MessageId = MessageId(0x4b2);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_DISALLOWED_API_CALLED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_DISALLOWED_API_CALLED: MessageId = MessageId(0x4b3);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_CANNOT_NEST_RENDER_PASSES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_CANNOT_NEST_RENDER_PASSES: MessageId = MessageId(0x4b4);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_CANNOT_END_WITHOUT_BEGIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_CANNOT_END_WITHOUT_BEGIN: MessageId = MessageId(0x4b5);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_CANNOT_CLOSE_COMMAND_LIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_CANNOT_CLOSE_COMMAND_LIST: MessageId = MessageId(0x4b6);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_GPU_WORK_WHILE_SUSPENDED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_GPU_WORK_WHILE_SUSPENDED: MessageId = MessageId(0x4b7);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_MISMATCHING_SUSPEND_RESUME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_MISMATCHING_SUSPEND_RESUME: MessageId = MessageId(0x4b8);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_NO_PRIOR_SUSPEND_WITHIN_EXECUTECOMMANDLISTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_NO_PRIOR_SUSPEND_WITHIN_EXECUTECOMMANDLISTS: MessageId =
    MessageId(0x4b9);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_NO_SUBSEQUENT_RESUME_WITHIN_EXECUTECOMMANDLISTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_NO_SUBSEQUENT_RESUME_WITHIN_EXECUTECOMMANDLISTS: MessageId =
    MessageId(0x4ba);
/// The header's `D3D12_MESSAGE_ID_TRACKED_WORKLOAD_COMMAND_QUEUE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_TRACKED_WORKLOAD_COMMAND_QUEUE_MISMATCH: MessageId = MessageId(0x4bb);
/// The header's `D3D12_MESSAGE_ID_TRACKED_WORKLOAD_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_TRACKED_WORKLOAD_NOT_SUPPORTED: MessageId = MessageId(0x4bc);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_MISMATCHING_NO_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_MISMATCHING_NO_ACCESS: MessageId = MessageId(0x4bd);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_UNSUPPORTED_RESOLVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_UNSUPPORTED_RESOLVE: MessageId = MessageId(0x4be);
/// The header's `D3D12_MESSAGE_ID_CLEARUNORDEREDACCESSVIEW_INVALID_RESOURCE_PTR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CLEARUNORDEREDACCESSVIEW_INVALID_RESOURCE_PTR: MessageId =
    MessageId(0x4bf);
/// The header's `D3D12_MESSAGE_ID_WINDOWS7_FENCE_OUTOFORDER_SIGNAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WINDOWS7_FENCE_OUTOFORDER_SIGNAL: MessageId = MessageId(0x4c0);
/// The header's `D3D12_MESSAGE_ID_WINDOWS7_FENCE_OUTOFORDER_WAIT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WINDOWS7_FENCE_OUTOFORDER_WAIT: MessageId = MessageId(0x4c1);
/// The header's `D3D12_MESSAGE_ID_VIDEO_CREATE_MOTION_ESTIMATOR_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_CREATE_MOTION_ESTIMATOR_INVALID_ARGUMENT: MessageId =
    MessageId(0x4c2);
/// The header's `D3D12_MESSAGE_ID_VIDEO_CREATE_MOTION_VECTOR_HEAP_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_CREATE_MOTION_VECTOR_HEAP_INVALID_ARGUMENT: MessageId =
    MessageId(0x4c3);
/// The header's `D3D12_MESSAGE_ID_ESTIMATE_MOTION_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ESTIMATE_MOTION_INVALID_ARGUMENT: MessageId = MessageId(0x4c4);
/// The header's `D3D12_MESSAGE_ID_RESOLVE_MOTION_VECTOR_HEAP_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVE_MOTION_VECTOR_HEAP_INVALID_ARGUMENT: MessageId =
    MessageId(0x4c5);
/// The header's `D3D12_MESSAGE_ID_GETGPUVIRTUALADDRESS_INVALID_HEAP_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETGPUVIRTUALADDRESS_INVALID_HEAP_TYPE: MessageId = MessageId(0x4c6);
/// The header's `D3D12_MESSAGE_ID_SET_BACKGROUND_PROCESSING_MODE_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_BACKGROUND_PROCESSING_MODE_INVALID_ARGUMENT: MessageId =
    MessageId(0x4c7);
/// The header's `D3D12_MESSAGE_ID_CREATE_COMMAND_LIST_INVALID_COMMAND_LIST_TYPE_FOR_FEATURE_LEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_COMMAND_LIST_INVALID_COMMAND_LIST_TYPE_FOR_FEATURE_LEVEL:
    MessageId = MessageId(0x4c8);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOEXTENSIONCOMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOEXTENSIONCOMMAND: MessageId = MessageId(0x4c9);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOEXTENSIONCOMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOEXTENSIONCOMMAND: MessageId = MessageId(0x4ca);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOEXTENSIONCOMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOEXTENSIONCOMMAND: MessageId = MessageId(0x4cb);
/// The header's `D3D12_MESSAGE_ID_INVALID_VIDEO_EXTENSION_COMMAND_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INVALID_VIDEO_EXTENSION_COMMAND_ID: MessageId = MessageId(0x4cc);
/// The header's `D3D12_MESSAGE_ID_VIDEO_EXTENSION_COMMAND_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VIDEO_EXTENSION_COMMAND_INVALID_ARGUMENT: MessageId = MessageId(0x4cd);
/// The header's `D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_NOT_UNIQUE_IN_DXIL_LIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_NOT_UNIQUE_IN_DXIL_LIBRARY: MessageId =
    MessageId(0x4ce);
/// The header's `D3D12_MESSAGE_ID_VARIABLE_SHADING_RATE_NOT_ALLOWED_WITH_TIR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VARIABLE_SHADING_RATE_NOT_ALLOWED_WITH_TIR: MessageId = MessageId(0x4cf);
/// The header's `D3D12_MESSAGE_ID_GEOMETRY_SHADER_OUTPUTTING_BOTH_VIEWPORT_ARRAY_INDEX_AND_SHADING_RATE_NOT_SUPPORTED_ON_DEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GEOMETRY_SHADER_OUTPUTTING_BOTH_VIEWPORT_ARRAY_INDEX_AND_SHADING_RATE_NOT_SUPPORTED_ON_DEVICE: MessageId = MessageId(0x4d0);
/// The header's `D3D12_MESSAGE_ID_RSSETSHADING_RATE_INVALID_SHADING_RATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RSSETSHADING_RATE_INVALID_SHADING_RATE: MessageId = MessageId(0x4d1);
/// The header's `D3D12_MESSAGE_ID_RSSETSHADING_RATE_SHADING_RATE_NOT_PERMITTED_BY_CAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RSSETSHADING_RATE_SHADING_RATE_NOT_PERMITTED_BY_CAP: MessageId =
    MessageId(0x4d2);
/// The header's `D3D12_MESSAGE_ID_RSSETSHADING_RATE_INVALID_COMBINER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RSSETSHADING_RATE_INVALID_COMBINER: MessageId = MessageId(0x4d3);
/// The header's `D3D12_MESSAGE_ID_RSSETSHADINGRATEIMAGE_REQUIRES_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RSSETSHADINGRATEIMAGE_REQUIRES_TIER_2: MessageId = MessageId(0x4d4);
/// The header's `D3D12_MESSAGE_ID_RSSETSHADINGRATE_REQUIRES_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RSSETSHADINGRATE_REQUIRES_TIER_1: MessageId = MessageId(0x4d5);
/// The header's `D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_FORMAT: MessageId = MessageId(0x4d6);
/// The header's `D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_ARRAY_SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_ARRAY_SIZE: MessageId = MessageId(0x4d7);
/// The header's `D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_MIP_LEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_MIP_LEVEL: MessageId = MessageId(0x4d8);
/// The header's `D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_SAMPLE_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_SAMPLE_COUNT: MessageId = MessageId(0x4d9);
/// The header's `D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_SAMPLE_QUALITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADING_RATE_IMAGE_INCORRECT_SAMPLE_QUALITY: MessageId =
    MessageId(0x4da);
/// The header's `D3D12_MESSAGE_ID_NON_RETAIL_SHADER_MODEL_WONT_VALIDATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_NON_RETAIL_SHADER_MODEL_WONT_VALIDATE: MessageId = MessageId(0x4db);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_AS_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_AS_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x4dc);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MS_ROOT_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MS_ROOT_SIGNATURE_MISMATCH: MessageId =
    MessageId(0x4dd);
/// The header's `D3D12_MESSAGE_ID_ADD_TO_STATE_OBJECT_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ADD_TO_STATE_OBJECT_ERROR: MessageId = MessageId(0x4de);
/// The header's `D3D12_MESSAGE_ID_CREATE_PROTECTED_RESOURCE_SESSION_INVALID_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_PROTECTED_RESOURCE_SESSION_INVALID_ARGUMENT: MessageId =
    MessageId(0x4df);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MS_PSO_DESC_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MS_PSO_DESC_MISMATCH: MessageId =
    MessageId(0x4e0);
/// The header's `D3D12_MESSAGE_ID_CREATEPIPELINESTATE_MS_INCOMPLETE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEPIPELINESTATE_MS_INCOMPLETE_TYPE: MessageId = MessageId(0x4e1);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_AS_NOT_MS_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_AS_NOT_MS_MISMATCH: MessageId =
    MessageId(0x4e2);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MS_NOT_PS_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_MS_NOT_PS_MISMATCH: MessageId =
    MessageId(0x4e3);
/// The header's `D3D12_MESSAGE_ID_NONZERO_SAMPLER_FEEDBACK_MIP_REGION_WITH_INCOMPATIBLE_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_NONZERO_SAMPLER_FEEDBACK_MIP_REGION_WITH_INCOMPATIBLE_FORMAT: MessageId =
    MessageId(0x4e4);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INPUTLAYOUT_SHADER_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_INPUTLAYOUT_SHADER_MISMATCH: MessageId =
    MessageId(0x4e5);
/// The header's `D3D12_MESSAGE_ID_EMPTY_DISPATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EMPTY_DISPATCH: MessageId = MessageId(0x4e6);
/// The header's `D3D12_MESSAGE_ID_RESOURCE_FORMAT_REQUIRES_SAMPLER_FEEDBACK_CAPABILITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOURCE_FORMAT_REQUIRES_SAMPLER_FEEDBACK_CAPABILITY: MessageId =
    MessageId(0x4e7);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_MIP_REGION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_MIP_REGION: MessageId = MessageId(0x4e8);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_DIMENSION: MessageId = MessageId(0x4e9);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_SAMPLE_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_SAMPLE_COUNT: MessageId = MessageId(0x4ea);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_SAMPLE_QUALITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_SAMPLE_QUALITY: MessageId =
    MessageId(0x4eb);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_INVALID_LAYOUT: MessageId = MessageId(0x4ec);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_REQUIRES_UNORDERED_ACCESS_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_MAP_REQUIRES_UNORDERED_ACCESS_FLAG: MessageId =
    MessageId(0x4ed);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_CREATE_UAV_NULL_ARGUMENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_CREATE_UAV_NULL_ARGUMENTS: MessageId = MessageId(0x4ee);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_UAV_REQUIRES_SAMPLER_FEEDBACK_CAPABILITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_UAV_REQUIRES_SAMPLER_FEEDBACK_CAPABILITY: MessageId =
    MessageId(0x4ef);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_CREATE_UAV_REQUIRES_FEEDBACK_MAP_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_CREATE_UAV_REQUIRES_FEEDBACK_MAP_FORMAT: MessageId =
    MessageId(0x4f0);
/// The header's `D3D12_MESSAGE_ID_CREATEMESHSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEMESHSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(0x4f1);
/// The header's `D3D12_MESSAGE_ID_CREATEMESHSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEMESHSHADER_OUTOFMEMORY: MessageId = MessageId(0x4f2);
/// The header's `D3D12_MESSAGE_ID_CREATEMESHSHADERWITHSTREAMOUTPUT_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEMESHSHADERWITHSTREAMOUTPUT_INVALIDSHADERTYPE: MessageId =
    MessageId(0x4f3);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_SAMPLER_FEEDBACK_TRANSCODE_INVALID_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_SAMPLER_FEEDBACK_TRANSCODE_INVALID_FORMAT: MessageId =
    MessageId(0x4f4);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_SAMPLER_FEEDBACK_INVALID_MIP_LEVEL_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_SAMPLER_FEEDBACK_INVALID_MIP_LEVEL_COUNT: MessageId =
    MessageId(0x4f5);
/// The header's `D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_SAMPLER_FEEDBACK_TRANSCODE_ARRAY_SIZE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVESUBRESOURCE_SAMPLER_FEEDBACK_TRANSCODE_ARRAY_SIZE_MISMATCH:
    MessageId = MessageId(0x4f6);
/// The header's `D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_CREATE_UAV_MISMATCHING_TARGETED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SAMPLER_FEEDBACK_CREATE_UAV_MISMATCHING_TARGETED_RESOURCE: MessageId =
    MessageId(0x4f7);
/// The header's `D3D12_MESSAGE_ID_CREATEMESHSHADER_OUTPUTEXCEEDSMAXSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEMESHSHADER_OUTPUTEXCEEDSMAXSIZE: MessageId = MessageId(0x4f8);
/// The header's `D3D12_MESSAGE_ID_CREATEMESHSHADER_GROUPSHAREDEXCEEDSMAXSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEMESHSHADER_GROUPSHAREDEXCEEDSMAXSIZE: MessageId = MessageId(0x4f9);
/// The header's `D3D12_MESSAGE_ID_VERTEX_SHADER_OUTPUTTING_BOTH_VIEWPORT_ARRAY_INDEX_AND_SHADING_RATE_NOT_SUPPORTED_ON_DEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VERTEX_SHADER_OUTPUTTING_BOTH_VIEWPORT_ARRAY_INDEX_AND_SHADING_RATE_NOT_SUPPORTED_ON_DEVICE: MessageId = MessageId(0x4fa);
/// The header's `D3D12_MESSAGE_ID_MESH_SHADER_OUTPUTTING_BOTH_VIEWPORT_ARRAY_INDEX_AND_SHADING_RATE_NOT_SUPPORTED_ON_DEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_MESH_SHADER_OUTPUTTING_BOTH_VIEWPORT_ARRAY_INDEX_AND_SHADING_RATE_NOT_SUPPORTED_ON_DEVICE: MessageId = MessageId(0x4fb);
/// The header's `D3D12_MESSAGE_ID_CREATEMESHSHADER_MISMATCHEDASMSPAYLOADSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEMESHSHADER_MISMATCHEDASMSPAYLOADSIZE: MessageId = MessageId(0x4fc);
/// The header's `D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_UNBOUNDED_STATIC_DESCRIPTORS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_ROOT_SIGNATURE_UNBOUNDED_STATIC_DESCRIPTORS: MessageId =
    MessageId(0x4fd);
/// The header's `D3D12_MESSAGE_ID_CREATEAMPLIFICATIONSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEAMPLIFICATIONSHADER_INVALIDSHADERBYTECODE: MessageId =
    MessageId(0x4fe);
/// The header's `D3D12_MESSAGE_ID_CREATEAMPLIFICATIONSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEAMPLIFICATIONSHADER_OUTOFMEMORY: MessageId = MessageId(0x4ff);
/// The header's `D3D12_MESSAGE_ID_CREATE_SHADERCACHESESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_SHADERCACHESESSION: MessageId = MessageId(0x500);
/// The header's `D3D12_MESSAGE_ID_LIVE_SHADERCACHESESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_SHADERCACHESESSION: MessageId = MessageId(0x501);
/// The header's `D3D12_MESSAGE_ID_DESTROY_SHADERCACHESESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_SHADERCACHESESSION: MessageId = MessageId(0x502);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERCACHESESSION_INVALIDARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERCACHESESSION_INVALIDARGS: MessageId = MessageId(0x503);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERCACHESESSION_DISABLED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERCACHESESSION_DISABLED: MessageId = MessageId(0x504);
/// The header's `D3D12_MESSAGE_ID_CREATESHADERCACHESESSION_ALREADYOPEN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATESHADERCACHESESSION_ALREADYOPEN: MessageId = MessageId(0x505);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHECONTROL_DEVELOPERMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHECONTROL_DEVELOPERMODE: MessageId = MessageId(0x506);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHECONTROL_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHECONTROL_INVALIDFLAGS: MessageId = MessageId(0x507);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHECONTROL_STATEALREADYSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHECONTROL_STATEALREADYSET: MessageId = MessageId(0x508);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHECONTROL_IGNOREDFLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHECONTROL_IGNOREDFLAG: MessageId = MessageId(0x509);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHESESSION_STOREVALUE_ALREADYPRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHESESSION_STOREVALUE_ALREADYPRESENT: MessageId =
    MessageId(0x50a);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHESESSION_STOREVALUE_HASHCOLLISION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHESESSION_STOREVALUE_HASHCOLLISION: MessageId =
    MessageId(0x50b);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHESESSION_STOREVALUE_CACHEFULL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHESESSION_STOREVALUE_CACHEFULL: MessageId = MessageId(0x50c);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHESESSION_FINDVALUE_NOTFOUND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHESESSION_FINDVALUE_NOTFOUND: MessageId = MessageId(0x50d);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHESESSION_CORRUPT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHESESSION_CORRUPT: MessageId = MessageId(0x50e);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHESESSION_DISABLED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHESESSION_DISABLED: MessageId = MessageId(0x50f);
/// The header's `D3D12_MESSAGE_ID_OVERSIZED_DISPATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OVERSIZED_DISPATCH: MessageId = MessageId(0x510);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOENCODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOENCODER: MessageId = MessageId(0x511);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOENCODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOENCODER: MessageId = MessageId(0x512);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOENCODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOENCODER: MessageId = MessageId(0x513);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEOENCODERHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEOENCODERHEAP: MessageId = MessageId(0x514);
/// The header's `D3D12_MESSAGE_ID_LIVE_VIDEOENCODERHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LIVE_VIDEOENCODERHEAP: MessageId = MessageId(0x515);
/// The header's `D3D12_MESSAGE_ID_DESTROY_VIDEOENCODERHEAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESTROY_VIDEOENCODERHEAP: MessageId = MessageId(0x516);
/// The header's `D3D12_MESSAGE_ID_COPYTEXTUREREGION_MISMATCH_ENCODE_REFERENCE_ONLY_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYTEXTUREREGION_MISMATCH_ENCODE_REFERENCE_ONLY_FLAG: MessageId =
    MessageId(0x517);
/// The header's `D3D12_MESSAGE_ID_COPYRESOURCE_MISMATCH_ENCODE_REFERENCE_ONLY_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COPYRESOURCE_MISMATCH_ENCODE_REFERENCE_ONLY_FLAG: MessageId =
    MessageId(0x518);
/// The header's `D3D12_MESSAGE_ID_ENCODE_FRAME_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ENCODE_FRAME_INVALID_PARAMETERS: MessageId = MessageId(0x519);
/// The header's `D3D12_MESSAGE_ID_ENCODE_FRAME_UNSUPPORTED_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ENCODE_FRAME_UNSUPPORTED_PARAMETERS: MessageId = MessageId(0x51a);
/// The header's `D3D12_MESSAGE_ID_RESOLVE_ENCODER_OUTPUT_METADATA_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVE_ENCODER_OUTPUT_METADATA_INVALID_PARAMETERS: MessageId =
    MessageId(0x51b);
/// The header's `D3D12_MESSAGE_ID_RESOLVE_ENCODER_OUTPUT_METADATA_UNSUPPORTED_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVE_ENCODER_OUTPUT_METADATA_UNSUPPORTED_PARAMETERS: MessageId =
    MessageId(0x51c);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEO_ENCODER_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEO_ENCODER_INVALID_PARAMETERS: MessageId = MessageId(0x51d);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEO_ENCODER_UNSUPPORTED_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEO_ENCODER_UNSUPPORTED_PARAMETERS: MessageId =
    MessageId(0x51e);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEO_ENCODER_HEAP_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEO_ENCODER_HEAP_INVALID_PARAMETERS: MessageId =
    MessageId(0x51f);
/// The header's `D3D12_MESSAGE_ID_CREATE_VIDEO_ENCODER_HEAP_UNSUPPORTED_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_VIDEO_ENCODER_HEAP_UNSUPPORTED_PARAMETERS: MessageId =
    MessageId(0x520);
/// The header's `D3D12_MESSAGE_ID_CREATECOMMANDLIST_NULL_COMMANDALLOCATOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATECOMMANDLIST_NULL_COMMANDALLOCATOR: MessageId = MessageId(0x521);
/// The header's `D3D12_MESSAGE_ID_CLEAR_UNORDERED_ACCESS_VIEW_INVALID_DESCRIPTOR_HANDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CLEAR_UNORDERED_ACCESS_VIEW_INVALID_DESCRIPTOR_HANDLE: MessageId =
    MessageId(0x522);
/// The header's `D3D12_MESSAGE_ID_DESCRIPTOR_HEAP_NOT_SHADER_VISIBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESCRIPTOR_HEAP_NOT_SHADER_VISIBLE: MessageId = MessageId(0x523);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_BLENDOP_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_BLENDOP_WARNING: MessageId = MessageId(0x524);
/// The header's `D3D12_MESSAGE_ID_CREATEBLENDSTATE_BLENDOPALPHA_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEBLENDSTATE_BLENDOPALPHA_WARNING: MessageId = MessageId(0x525);
/// The header's `D3D12_MESSAGE_ID_WRITE_COMBINE_PERFORMANCE_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_WRITE_COMBINE_PERFORMANCE_WARNING: MessageId = MessageId(0x526);
/// The header's `D3D12_MESSAGE_ID_RESOLVE_QUERY_INVALID_QUERY_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVE_QUERY_INVALID_QUERY_STATE: MessageId = MessageId(0x527);
/// The header's `D3D12_MESSAGE_ID_SETPRIVATEDATA_NO_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETPRIVATEDATA_NO_ACCESS: MessageId = MessageId(0x528);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_STATIC_DESCRIPTOR_SAMPLER_MODE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_STATIC_DESCRIPTOR_SAMPLER_MODE_MISMATCH: MessageId =
    MessageId(0x529);
/// The header's `D3D12_MESSAGE_ID_GETCOPYABLEFOOTPRINTS_UNSUPPORTED_BUFFER_WIDTH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GETCOPYABLEFOOTPRINTS_UNSUPPORTED_BUFFER_WIDTH: MessageId =
    MessageId(0x52a);
/// The header's `D3D12_MESSAGE_ID_CREATEMESHSHADER_TOPOLOGY_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEMESHSHADER_TOPOLOGY_MISMATCH: MessageId = MessageId(0x52b);
/// The header's `D3D12_MESSAGE_ID_VRS_SUM_COMBINER_REQUIRES_CAPABILITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_VRS_SUM_COMBINER_REQUIRES_CAPABILITY: MessageId = MessageId(0x52c);
/// The header's `D3D12_MESSAGE_ID_SETTING_SHADING_RATE_FROM_MS_REQUIRES_CAPABILITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SETTING_SHADING_RATE_FROM_MS_REQUIRES_CAPABILITY: MessageId =
    MessageId(0x52d);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHESESSION_SHADERCACHEDELETE_NOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHESESSION_SHADERCACHEDELETE_NOTSUPPORTED: MessageId =
    MessageId(0x52e);
/// The header's `D3D12_MESSAGE_ID_SHADERCACHECONTROL_SHADERCACHECLEAR_NOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADERCACHECONTROL_SHADERCACHECLEAR_NOTSUPPORTED: MessageId =
    MessageId(0x52f);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_STATE_IGNORED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_STATE_IGNORED: MessageId = MessageId(0x530);
/// The header's `D3D12_MESSAGE_ID_UNUSED_CROSS_EXECUTE_SPLIT_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNUSED_CROSS_EXECUTE_SPLIT_BARRIER: MessageId = MessageId(0x531);
/// The header's `D3D12_MESSAGE_ID_DEVICE_OPEN_SHARED_HANDLE_ACCESS_DENIED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DEVICE_OPEN_SHARED_HANDLE_ACCESS_DENIED: MessageId = MessageId(0x532);
/// The header's `D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_VALUES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_VALUES: MessageId = MessageId(0x533);
/// The header's `D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_ACCESS: MessageId = MessageId(0x534);
/// The header's `D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_SYNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_SYNC: MessageId = MessageId(0x535);
/// The header's `D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_LAYOUT: MessageId = MessageId(0x536);
/// The header's `D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_TYPE: MessageId = MessageId(0x537);
/// The header's `D3D12_MESSAGE_ID_OUT_OF_BOUNDS_BARRIER_SUBRESOURCE_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_OUT_OF_BOUNDS_BARRIER_SUBRESOURCE_RANGE: MessageId = MessageId(0x538);
/// The header's `D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_RESOURCE_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INCOMPATIBLE_BARRIER_RESOURCE_DIMENSION: MessageId = MessageId(0x539);
/// The header's `D3D12_MESSAGE_ID_SET_SCISSOR_RECTS_INVALID_RECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SET_SCISSOR_RECTS_INVALID_RECT: MessageId = MessageId(0x53a);
/// The header's `D3D12_MESSAGE_ID_SHADING_RATE_SOURCE_REQUIRES_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_SHADING_RATE_SOURCE_REQUIRES_DIMENSION_TEXTURE2D: MessageId =
    MessageId(0x53b);
/// The header's `D3D12_MESSAGE_ID_BUFFER_BARRIER_SUBREGION_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_BUFFER_BARRIER_SUBREGION_OUT_OF_BOUNDS: MessageId = MessageId(0x53c);
/// The header's `D3D12_MESSAGE_ID_UNSUPPORTED_BARRIER_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_UNSUPPORTED_BARRIER_LAYOUT: MessageId = MessageId(0x53d);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALID_PARAMETERS: MessageId = MessageId(0x53e);
/// The header's `D3D12_MESSAGE_ID_ENHANCED_BARRIERS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ENHANCED_BARRIERS_NOT_SUPPORTED: MessageId = MessageId(0x53f);
/// The header's `D3D12_MESSAGE_ID_LEGACY_BARRIER_VALIDATION_FORCED_ON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_LEGACY_BARRIER_VALIDATION_FORCED_ON: MessageId = MessageId(0x542);
/// The header's `D3D12_MESSAGE_ID_EMPTY_ROOT_DESCRIPTOR_TABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EMPTY_ROOT_DESCRIPTOR_TABLE: MessageId = MessageId(0x543);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_ELEMENT_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_ELEMENT_OFFSET_UNALIGNED: MessageId = MessageId(0x544);
/// The header's `D3D12_MESSAGE_ID_ALPHA_BLEND_FACTOR_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_ALPHA_BLEND_FACTOR_NOT_SUPPORTED: MessageId = MessageId(0x545);
/// The header's `D3D12_MESSAGE_ID_BARRIER_INTEROP_INVALID_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_BARRIER_INTEROP_INVALID_LAYOUT: MessageId = MessageId(0x546);
/// The header's `D3D12_MESSAGE_ID_BARRIER_INTEROP_INVALID_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_BARRIER_INTEROP_INVALID_STATE: MessageId = MessageId(0x547);
/// The header's `D3D12_MESSAGE_ID_GRAPHICS_PIPELINE_STATE_DESC_ZERO_SAMPLE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GRAPHICS_PIPELINE_STATE_DESC_ZERO_SAMPLE_MASK: MessageId =
    MessageId(0x548);
/// The header's `D3D12_MESSAGE_ID_INDEPENDENT_STENCIL_REF_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INDEPENDENT_STENCIL_REF_NOT_SUPPORTED: MessageId = MessageId(0x549);
/// The header's `D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INDEPENDENT_MASKS_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INDEPENDENT_MASKS_UNSUPPORTED: MessageId =
    MessageId(0x54a);
/// The header's `D3D12_MESSAGE_ID_TEXTURE_BARRIER_SUBRESOURCES_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_TEXTURE_BARRIER_SUBRESOURCES_OUT_OF_BOUNDS: MessageId = MessageId(0x54b);
/// The header's `D3D12_MESSAGE_ID_NON_OPTIMAL_BARRIER_ONLY_EXECUTE_COMMAND_LISTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_NON_OPTIMAL_BARRIER_ONLY_EXECUTE_COMMAND_LISTS: MessageId =
    MessageId(0x54c);
/// The header's `D3D12_MESSAGE_ID_EXECUTE_INDIRECT_ZERO_COMMAND_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_EXECUTE_INDIRECT_ZERO_COMMAND_COUNT: MessageId = MessageId(0x54d);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_INCOMPATIBLE_TEXTURE_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_INCOMPATIBLE_TEXTURE_LAYOUT: MessageId =
    MessageId(0x54e);
/// The header's `D3D12_MESSAGE_ID_DYNAMIC_INDEX_BUFFER_STRIP_CUT_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DYNAMIC_INDEX_BUFFER_STRIP_CUT_NOT_SUPPORTED: MessageId =
    MessageId(0x54f);
/// The header's `D3D12_MESSAGE_ID_PRIMITIVE_TOPOLOGY_TRIANGLE_FANS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PRIMITIVE_TOPOLOGY_TRIANGLE_FANS_NOT_SUPPORTED: MessageId =
    MessageId(0x550);
/// The header's `D3D12_MESSAGE_ID_CREATE_SAMPLER_COMPARISON_FUNC_IGNORED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_SAMPLER_COMPARISON_FUNC_IGNORED: MessageId = MessageId(0x551);
/// The header's `D3D12_MESSAGE_ID_CREATEHEAP_INVALIDHEAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEHEAP_INVALIDHEAPTYPE: MessageId = MessageId(0x552);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALIDHEAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCEANDHEAP_INVALIDHEAPTYPE: MessageId = MessageId(0x553);
/// The header's `D3D12_MESSAGE_ID_DYNAMIC_DEPTH_BIAS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DYNAMIC_DEPTH_BIAS_NOT_SUPPORTED: MessageId = MessageId(0x554);
/// The header's `D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_NON_WHOLE_DYNAMIC_DEPTH_BIAS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_NON_WHOLE_DYNAMIC_DEPTH_BIAS: MessageId =
    MessageId(0x555);
/// The header's `D3D12_MESSAGE_ID_DYNAMIC_DEPTH_BIAS_FLAG_MISSING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DYNAMIC_DEPTH_BIAS_FLAG_MISSING: MessageId = MessageId(0x556);
/// The header's `D3D12_MESSAGE_ID_DYNAMIC_DEPTH_BIAS_NO_PIPELINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DYNAMIC_DEPTH_BIAS_NO_PIPELINE: MessageId = MessageId(0x557);
/// The header's `D3D12_MESSAGE_ID_DYNAMIC_INDEX_BUFFER_STRIP_CUT_FLAG_MISSING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DYNAMIC_INDEX_BUFFER_STRIP_CUT_FLAG_MISSING: MessageId =
    MessageId(0x558);
/// The header's `D3D12_MESSAGE_ID_DYNAMIC_INDEX_BUFFER_STRIP_CUT_NO_PIPELINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DYNAMIC_INDEX_BUFFER_STRIP_CUT_NO_PIPELINE: MessageId = MessageId(0x559);
/// The header's `D3D12_MESSAGE_ID_NONNORMALIZED_COORDINATE_SAMPLING_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_NONNORMALIZED_COORDINATE_SAMPLING_NOT_SUPPORTED: MessageId =
    MessageId(0x55a);
/// The header's `D3D12_MESSAGE_ID_INVALID_CAST_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INVALID_CAST_TARGET: MessageId = MessageId(0x55b);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_COMMANDLIST_INVALID_END_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_COMMANDLIST_INVALID_END_STATE: MessageId = MessageId(0x55c);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_COMMANDLIST_INVALID_START_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_COMMANDLIST_INVALID_START_STATE: MessageId =
    MessageId(0x55d);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_MISMATCHING_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_MISMATCHING_ACCESS: MessageId = MessageId(0x55e);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_MISMATCHING_LOCAL_PRESERVE_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_MISMATCHING_LOCAL_PRESERVE_PARAMETERS: MessageId =
    MessageId(0x55f);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_LOCAL_PRESERVE_RENDER_PARAMETERS_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_LOCAL_PRESERVE_RENDER_PARAMETERS_ERROR: MessageId =
    MessageId(0x560);
/// The header's `D3D12_MESSAGE_ID_RENDER_PASS_LOCAL_DEPTH_STENCIL_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RENDER_PASS_LOCAL_DEPTH_STENCIL_ERROR: MessageId = MessageId(0x561);
/// The header's `D3D12_MESSAGE_ID_DRAW_POTENTIALLY_OUTSIDE_OF_VALID_RENDER_AREA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DRAW_POTENTIALLY_OUTSIDE_OF_VALID_RENDER_AREA: MessageId =
    MessageId(0x562);
/// The header's `D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALID_LINERASTERIZATIONMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERASTERIZERSTATE_INVALID_LINERASTERIZATIONMODE: MessageId =
    MessageId(0x563);
/// The header's `D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDALIGNMENT_SMALLRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATERESOURCE_INVALIDALIGNMENT_SMALLRESOURCE: MessageId =
    MessageId(0x564);
/// The header's `D3D12_MESSAGE_ID_GENERIC_DEVICE_OPERATION_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GENERIC_DEVICE_OPERATION_UNSUPPORTED: MessageId = MessageId(0x565);
/// The header's `D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_RENDER_TARGET_WRONG_WRITE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATEGRAPHICSPIPELINESTATE_RENDER_TARGET_WRONG_WRITE_MASK: MessageId =
    MessageId(0x566);
/// The header's `D3D12_MESSAGE_ID_PROBABLE_PIX_EVENT_LEAK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PROBABLE_PIX_EVENT_LEAK: MessageId = MessageId(0x567);
/// The header's `D3D12_MESSAGE_ID_PIX_EVENT_UNDERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_PIX_EVENT_UNDERFLOW: MessageId = MessageId(0x568);
/// The header's `D3D12_MESSAGE_ID_RECREATEAT_INVALID_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RECREATEAT_INVALID_TARGET: MessageId = MessageId(0x569);
/// The header's `D3D12_MESSAGE_ID_RECREATEAT_INSUFFICIENT_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RECREATEAT_INSUFFICIENT_SUPPORT: MessageId = MessageId(0x56a);
/// The header's `D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_STRUCTURED_BUFFER_STRIDE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GPU_BASED_VALIDATION_STRUCTURED_BUFFER_STRIDE_MISMATCH: MessageId =
    MessageId(0x56b);
/// The header's `D3D12_MESSAGE_ID_DISPATCH_GRAPH_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DISPATCH_GRAPH_INVALID: MessageId = MessageId(0x56c);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_TARGET_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_TARGET_FORMAT_INVALID: MessageId = MessageId(0x56d);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_TARGET_DIMENSION_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_TARGET_DIMENSION_INVALID: MessageId = MessageId(0x56e);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_SOURCE_COLOR_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_SOURCE_COLOR_FORMAT_INVALID: MessageId =
    MessageId(0x56f);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_SOURCE_DEPTH_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_SOURCE_DEPTH_FORMAT_INVALID: MessageId =
    MessageId(0x570);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXPOSURE_SCALE_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXPOSURE_SCALE_FORMAT_INVALID: MessageId =
    MessageId(0x571);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_ENGINE_CREATE_FLAGS_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_ENGINE_CREATE_FLAGS_INVALID: MessageId =
    MessageId(0x572);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXTENSION_INTERNAL_LOAD_FAILURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXTENSION_INTERNAL_LOAD_FAILURE: MessageId =
    MessageId(0x573);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXTENSION_INTERNAL_ENGINE_CREATION_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXTENSION_INTERNAL_ENGINE_CREATION_ERROR: MessageId =
    MessageId(0x574);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXTENSION_INTERNAL_UPSCALER_CREATION_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXTENSION_INTERNAL_UPSCALER_CREATION_ERROR: MessageId =
    MessageId(0x575);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXTENSION_INTERNAL_UPSCALER_EXECUTION_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_EXTENSION_INTERNAL_UPSCALER_EXECUTION_ERROR:
    MessageId = MessageId(0x576);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_REGION_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_REGION_INVALID: MessageId =
    MessageId(0x577);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_TIME_DELTA_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_TIME_DELTA_INVALID: MessageId =
    MessageId(0x578);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_REQUIRED_TEXTURE_IS_NULL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_REQUIRED_TEXTURE_IS_NULL: MessageId =
    MessageId(0x579);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_MOTION_VECTORS_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_MOTION_VECTORS_FORMAT_INVALID:
    MessageId = MessageId(0x57a);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_FLAGS_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_FLAGS_INVALID: MessageId =
    MessageId(0x57b);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_FORMAT_INVALID: MessageId =
    MessageId(0x57c);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_EXPOSURE_SCALE_TEXTURE_SIZE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_EXPOSURE_SCALE_TEXTURE_SIZE_INVALID: MessageId = MessageId(0x57d);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_VARIANT_INDEX_OUT_OF_BOUNDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_VARIANT_INDEX_OUT_OF_BOUNDS: MessageId =
    MessageId(0x57e);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_VARIANT_ID_NOT_FOUND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_VARIANT_ID_NOT_FOUND: MessageId = MessageId(0x57f);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_DUPLICATE_VARIANT_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_DUPLICATE_VARIANT_ID: MessageId = MessageId(0x580);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_OUT_OF_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_OUT_OF_MEMORY: MessageId = MessageId(0x581);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_UNEXPECTED_TEXTURE_IS_IGNORED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_UNEXPECTED_TEXTURE_IS_IGNORED:
    MessageId = MessageId(0x582);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EVICT_UNDERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EVICT_UNDERFLOW: MessageId = MessageId(0x583);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_OPTIONAL_TEXTURE_IS_NULL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_OPTIONAL_TEXTURE_IS_NULL: MessageId =
    MessageId(0x584);
/// The header's `D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_INVALID_CAMERA_JITTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIRECTSR_SUPERRES_UPSCALER_EXECUTE_INVALID_CAMERA_JITTER: MessageId =
    MessageId(0x585);
/// The header's `D3D12_MESSAGE_ID_CREATE_STATE_OBJECT_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_CREATE_STATE_OBJECT_WARNING: MessageId = MessageId(0x586);
/// The header's `D3D12_MESSAGE_ID_GUID_TEXTURE_LAYOUT_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_GUID_TEXTURE_LAYOUT_UNSUPPORTED: MessageId = MessageId(0x587);
/// The header's `D3D12_MESSAGE_ID_RESOLVE_ENCODER_INPUT_PARAM_LAYOUT_INVALID_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_RESOLVE_ENCODER_INPUT_PARAM_LAYOUT_INVALID_PARAMETERS: MessageId =
    MessageId(0x588);
/// The header's `D3D12_MESSAGE_ID_INVALID_BARRIER_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_INVALID_BARRIER_ACCESS: MessageId = MessageId(0x589);
/// The header's `D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INSTANCE_COUNT_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_COMMAND_LIST_DRAW_INSTANCE_COUNT_ZERO: MessageId = MessageId(0x58a);
/// The header's `D3D12_MESSAGE_ID_DESCRIPTOR_HEAP_NOT_SET_BEFORE_ROOT_SIGNATURE_WITH_DIRECTLY_INDEXED_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DESCRIPTOR_HEAP_NOT_SET_BEFORE_ROOT_SIGNATURE_WITH_DIRECTLY_INDEXED_FLAG: MessageId = MessageId(0x58b);
/// The header's `D3D12_MESSAGE_ID_DIFFERENT_DESCRIPTOR_HEAP_SET_AFTER_ROOT_SIGNATURE_WITH_DIRECTLY_INDEXED_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_DIFFERENT_DESCRIPTOR_HEAP_SET_AFTER_ROOT_SIGNATURE_WITH_DIRECTLY_INDEXED_FLAG: MessageId = MessageId(0x58c);
/// The header's `D3D12_MESSAGE_ID_D3D12_MESSAGES_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_id).
pub const D3D12_MESSAGE_ID_D3D12_MESSAGES_END: MessageId = MessageId(0x58d);

/// The header's `D3D12_MESSAGE_SEVERITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_severity).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MessageSeverity(pub i32);

/// The header's `D3D12_MESSAGE_SEVERITY_CORRUPTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_severity).
pub const D3D12_MESSAGE_SEVERITY_CORRUPTION: MessageSeverity = MessageSeverity(0);
/// The header's `D3D12_MESSAGE_SEVERITY_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_severity).
pub const D3D12_MESSAGE_SEVERITY_ERROR: MessageSeverity = MessageSeverity(1);
/// The header's `D3D12_MESSAGE_SEVERITY_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_severity).
pub const D3D12_MESSAGE_SEVERITY_WARNING: MessageSeverity = MessageSeverity(2);
/// The header's `D3D12_MESSAGE_SEVERITY_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_severity).
pub const D3D12_MESSAGE_SEVERITY_INFO: MessageSeverity = MessageSeverity(3);
/// The header's `D3D12_MESSAGE_SEVERITY_MESSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_message_severity).
pub const D3D12_MESSAGE_SEVERITY_MESSAGE: MessageSeverity = MessageSeverity(4);

/// The header's `D3D12_META_COMMAND_PARAMETER_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MetaCommandParameterFlags(pub i32);

/// The header's `D3D12_META_COMMAND_PARAMETER_FLAG_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_flags).
pub const D3D12_META_COMMAND_PARAMETER_FLAG_INPUT: MetaCommandParameterFlags =
    MetaCommandParameterFlags(1);
/// The header's `D3D12_META_COMMAND_PARAMETER_FLAG_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_flags).
pub const D3D12_META_COMMAND_PARAMETER_FLAG_OUTPUT: MetaCommandParameterFlags =
    MetaCommandParameterFlags(2);

/// The header's `D3D12_META_COMMAND_PARAMETER_STAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_stage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MetaCommandParameterStage(pub i32);

/// The header's `D3D12_META_COMMAND_PARAMETER_STAGE_CREATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_stage).
pub const D3D12_META_COMMAND_PARAMETER_STAGE_CREATION: MetaCommandParameterStage =
    MetaCommandParameterStage(0);
/// The header's `D3D12_META_COMMAND_PARAMETER_STAGE_INITIALIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_stage).
pub const D3D12_META_COMMAND_PARAMETER_STAGE_INITIALIZATION: MetaCommandParameterStage =
    MetaCommandParameterStage(1);
/// The header's `D3D12_META_COMMAND_PARAMETER_STAGE_EXECUTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_stage).
pub const D3D12_META_COMMAND_PARAMETER_STAGE_EXECUTION: MetaCommandParameterStage =
    MetaCommandParameterStage(2);

/// The header's `D3D12_META_COMMAND_PARAMETER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MetaCommandParameterType(pub i32);

/// The header's `D3D12_META_COMMAND_PARAMETER_TYPE_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_type).
pub const D3D12_META_COMMAND_PARAMETER_TYPE_FLOAT: MetaCommandParameterType =
    MetaCommandParameterType(0);
/// The header's `D3D12_META_COMMAND_PARAMETER_TYPE_UINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_type).
pub const D3D12_META_COMMAND_PARAMETER_TYPE_UINT64: MetaCommandParameterType =
    MetaCommandParameterType(1);
/// The header's `D3D12_META_COMMAND_PARAMETER_TYPE_GPU_VIRTUAL_ADDRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_type).
pub const D3D12_META_COMMAND_PARAMETER_TYPE_GPU_VIRTUAL_ADDRESS: MetaCommandParameterType =
    MetaCommandParameterType(2);
/// The header's `D3D12_META_COMMAND_PARAMETER_TYPE_CPU_DESCRIPTOR_HANDLE_HEAP_TYPE_CBV_SRV_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_type).
pub const D3D12_META_COMMAND_PARAMETER_TYPE_CPU_DESCRIPTOR_HANDLE_HEAP_TYPE_CBV_SRV_UAV:
    MetaCommandParameterType = MetaCommandParameterType(3);
/// The header's `D3D12_META_COMMAND_PARAMETER_TYPE_GPU_DESCRIPTOR_HANDLE_HEAP_TYPE_CBV_SRV_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_meta_command_parameter_type).
pub const D3D12_META_COMMAND_PARAMETER_TYPE_GPU_DESCRIPTOR_HANDLE_HEAP_TYPE_CBV_SRV_UAV:
    MetaCommandParameterType = MetaCommandParameterType(4);

/// The header's `D3D12_MULTIPLE_FENCE_WAIT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_multiple_fence_wait_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MultipleFenceWaitFlags(pub i32);

/// The header's `D3D12_MULTIPLE_FENCE_WAIT_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_multiple_fence_wait_flags).
pub const D3D12_MULTIPLE_FENCE_WAIT_FLAG_NONE: MultipleFenceWaitFlags = MultipleFenceWaitFlags(0);
/// The header's `D3D12_MULTIPLE_FENCE_WAIT_FLAG_ANY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_multiple_fence_wait_flags).
pub const D3D12_MULTIPLE_FENCE_WAIT_FLAG_ANY: MultipleFenceWaitFlags = MultipleFenceWaitFlags(1);
/// The header's `D3D12_MULTIPLE_FENCE_WAIT_FLAG_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_multiple_fence_wait_flags).
pub const D3D12_MULTIPLE_FENCE_WAIT_FLAG_ALL: MultipleFenceWaitFlags = MultipleFenceWaitFlags(0);

/// The header's `D3D12_MULTISAMPLE_QUALITY_LEVEL_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_multisample_quality_level_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MultisampleQualityLevelFlags(pub i32);

/// The header's `D3D12_MULTISAMPLE_QUALITY_LEVELS_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_multisample_quality_level_flags).
pub const D3D12_MULTISAMPLE_QUALITY_LEVELS_FLAG_NONE: MultisampleQualityLevelFlags =
    MultisampleQualityLevelFlags(0);
/// The header's `D3D12_MULTISAMPLE_QUALITY_LEVELS_FLAG_TILED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_multisample_quality_level_flags).
pub const D3D12_MULTISAMPLE_QUALITY_LEVELS_FLAG_TILED_RESOURCE: MultisampleQualityLevelFlags =
    MultisampleQualityLevelFlags(1);

/// The header's `D3D12_NODE_OVERRIDES_TYPE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NodeOverridesType(pub i32);

/// The header's `D3D12_NODE_OVERRIDES_TYPE_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_NODE_OVERRIDES_TYPE_NONE: NodeOverridesType = NodeOverridesType(0);
/// The header's `D3D12_NODE_OVERRIDES_TYPE_BROADCASTING_LAUNCH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_NODE_OVERRIDES_TYPE_BROADCASTING_LAUNCH: NodeOverridesType = NodeOverridesType(1);
/// The header's `D3D12_NODE_OVERRIDES_TYPE_COALESCING_LAUNCH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_NODE_OVERRIDES_TYPE_COALESCING_LAUNCH: NodeOverridesType = NodeOverridesType(2);
/// The header's `D3D12_NODE_OVERRIDES_TYPE_THREAD_LAUNCH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_NODE_OVERRIDES_TYPE_THREAD_LAUNCH: NodeOverridesType = NodeOverridesType(3);
/// The header's `D3D12_NODE_OVERRIDES_TYPE_COMMON_COMPUTE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_NODE_OVERRIDES_TYPE_COMMON_COMPUTE: NodeOverridesType = NodeOverridesType(4);

/// The header's `D3D12_NODE_TYPE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NodeType(pub i32);

/// The header's `D3D12_NODE_TYPE_SHADER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_NODE_TYPE_SHADER: NodeType = NodeType(0);

/// The header's `D3D12_PIPELINE_STATE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PipelineStateFlags(pub i32);

/// The header's `D3D12_PIPELINE_STATE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_flags).
pub const D3D12_PIPELINE_STATE_FLAG_NONE: PipelineStateFlags = PipelineStateFlags(0);
/// The header's `D3D12_PIPELINE_STATE_FLAG_TOOL_DEBUG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_flags).
pub const D3D12_PIPELINE_STATE_FLAG_TOOL_DEBUG: PipelineStateFlags = PipelineStateFlags(1);
/// The header's `D3D12_PIPELINE_STATE_FLAG_DYNAMIC_DEPTH_BIAS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_flags).
pub const D3D12_PIPELINE_STATE_FLAG_DYNAMIC_DEPTH_BIAS: PipelineStateFlags = PipelineStateFlags(4);
/// The header's `D3D12_PIPELINE_STATE_FLAG_DYNAMIC_INDEX_BUFFER_STRIP_CUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_flags).
pub const D3D12_PIPELINE_STATE_FLAG_DYNAMIC_INDEX_BUFFER_STRIP_CUT: PipelineStateFlags =
    PipelineStateFlags(8);

/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PipelineStateSubobjectType(pub i32);

/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_ROOT_SIGNATURE: PipelineStateSubobjectType =
    PipelineStateSubobjectType(0);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(1);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(2);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(3);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_HS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_HS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(4);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_GS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_GS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(5);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(6);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_STREAM_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_STREAM_OUTPUT: PipelineStateSubobjectType =
    PipelineStateSubobjectType(7);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_BLEND: PipelineStateSubobjectType =
    PipelineStateSubobjectType(8);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_MASK: PipelineStateSubobjectType =
    PipelineStateSubobjectType(9);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER: PipelineStateSubobjectType =
    PipelineStateSubobjectType(10);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL: PipelineStateSubobjectType =
    PipelineStateSubobjectType(11);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_INPUT_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_INPUT_LAYOUT: PipelineStateSubobjectType =
    PipelineStateSubobjectType(12);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_IB_STRIP_CUT_VALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_IB_STRIP_CUT_VALUE: PipelineStateSubobjectType =
    PipelineStateSubobjectType(13);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY: PipelineStateSubobjectType =
    PipelineStateSubobjectType(14);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(15);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT: PipelineStateSubobjectType =
    PipelineStateSubobjectType(16);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_DESC: PipelineStateSubobjectType =
    PipelineStateSubobjectType(17);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_NODE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_NODE_MASK: PipelineStateSubobjectType =
    PipelineStateSubobjectType(18);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CACHED_PSO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CACHED_PSO: PipelineStateSubobjectType =
    PipelineStateSubobjectType(19);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_FLAGS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(20);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1: PipelineStateSubobjectType =
    PipelineStateSubobjectType(21);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING: PipelineStateSubobjectType =
    PipelineStateSubobjectType(22);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_AS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_AS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(24);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_MS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_MS: PipelineStateSubobjectType =
    PipelineStateSubobjectType(25);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL2: PipelineStateSubobjectType =
    PipelineStateSubobjectType(26);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER1: PipelineStateSubobjectType =
    PipelineStateSubobjectType(27);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER2: PipelineStateSubobjectType =
    PipelineStateSubobjectType(28);
/// The header's `D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_MAX_VALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_pipeline_state_subobject_type).
pub const D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_MAX_VALID: PipelineStateSubobjectType =
    PipelineStateSubobjectType(29);

/// The header's `D3D12_PREDICATION_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_predication_op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PredicationOp(pub i32);

/// The header's `D3D12_PREDICATION_OP_EQUAL_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_predication_op).
pub const D3D12_PREDICATION_OP_EQUAL_ZERO: PredicationOp = PredicationOp(0);
/// The header's `D3D12_PREDICATION_OP_NOT_EQUAL_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_predication_op).
pub const D3D12_PREDICATION_OP_NOT_EQUAL_ZERO: PredicationOp = PredicationOp(1);

/// The header's `D3D12_PRIMITIVE_TOPOLOGY_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_primitive_topology_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PrimitiveTopologyType(pub i32);

/// The header's `D3D12_PRIMITIVE_TOPOLOGY_TYPE_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_primitive_topology_type).
pub const D3D12_PRIMITIVE_TOPOLOGY_TYPE_UNDEFINED: PrimitiveTopologyType = PrimitiveTopologyType(0);
/// The header's `D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_primitive_topology_type).
pub const D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT: PrimitiveTopologyType = PrimitiveTopologyType(1);
/// The header's `D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_primitive_topology_type).
pub const D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE: PrimitiveTopologyType = PrimitiveTopologyType(2);
/// The header's `D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_primitive_topology_type).
pub const D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE: PrimitiveTopologyType = PrimitiveTopologyType(3);
/// The header's `D3D12_PRIMITIVE_TOPOLOGY_TYPE_PATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_primitive_topology_type).
pub const D3D12_PRIMITIVE_TOPOLOGY_TYPE_PATCH: PrimitiveTopologyType = PrimitiveTopologyType(4);

/// The header's `D3D12_PROGRAMMABLE_SAMPLE_POSITIONS_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_programmable_sample_positions_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ProgrammableSamplePositionsTier(pub i32);

/// The header's `D3D12_PROGRAMMABLE_SAMPLE_POSITIONS_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_programmable_sample_positions_tier).
pub const D3D12_PROGRAMMABLE_SAMPLE_POSITIONS_TIER_NOT_SUPPORTED: ProgrammableSamplePositionsTier =
    ProgrammableSamplePositionsTier(0);
/// The header's `D3D12_PROGRAMMABLE_SAMPLE_POSITIONS_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_programmable_sample_positions_tier).
pub const D3D12_PROGRAMMABLE_SAMPLE_POSITIONS_TIER_1: ProgrammableSamplePositionsTier =
    ProgrammableSamplePositionsTier(1);
/// The header's `D3D12_PROGRAMMABLE_SAMPLE_POSITIONS_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_programmable_sample_positions_tier).
pub const D3D12_PROGRAMMABLE_SAMPLE_POSITIONS_TIER_2: ProgrammableSamplePositionsTier =
    ProgrammableSamplePositionsTier(2);

/// The header's `D3D12_PROGRAM_TYPE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ProgramType(pub i32);

/// The header's `D3D12_PROGRAM_TYPE_GENERIC_PIPELINE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_PROGRAM_TYPE_GENERIC_PIPELINE: ProgramType = ProgramType(1);
/// The header's `D3D12_PROGRAM_TYPE_RAYTRACING_PIPELINE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_PROGRAM_TYPE_RAYTRACING_PIPELINE: ProgramType = ProgramType(4);
/// The header's `D3D12_PROGRAM_TYPE_WORK_GRAPH`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_PROGRAM_TYPE_WORK_GRAPH: ProgramType = ProgramType(5);

/// The header's `D3D12_PROTECTED_RESOURCE_SESSION_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_protected_resource_session_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ProtectedResourceSessionFlags(pub i32);

/// The header's `D3D12_PROTECTED_RESOURCE_SESSION_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_protected_resource_session_flags).
pub const D3D12_PROTECTED_RESOURCE_SESSION_FLAG_NONE: ProtectedResourceSessionFlags =
    ProtectedResourceSessionFlags(0);

/// The header's `D3D12_PROTECTED_RESOURCE_SESSION_SUPPORT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_protected_resource_session_support_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ProtectedResourceSessionSupportFlags(pub i32);

/// The header's `D3D12_PROTECTED_RESOURCE_SESSION_SUPPORT_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_protected_resource_session_support_flags).
pub const D3D12_PROTECTED_RESOURCE_SESSION_SUPPORT_FLAG_NONE: ProtectedResourceSessionSupportFlags =
    ProtectedResourceSessionSupportFlags(0);
/// The header's `D3D12_PROTECTED_RESOURCE_SESSION_SUPPORT_FLAG_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_protected_resource_session_support_flags).
pub const D3D12_PROTECTED_RESOURCE_SESSION_SUPPORT_FLAG_SUPPORTED:
    ProtectedResourceSessionSupportFlags = ProtectedResourceSessionSupportFlags(1);

/// The header's `D3D12_PROTECTED_SESSION_STATUS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_protected_session_status).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ProtectedSessionStatus(pub i32);

/// The header's `D3D12_PROTECTED_SESSION_STATUS_OK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_protected_session_status).
pub const D3D12_PROTECTED_SESSION_STATUS_OK: ProtectedSessionStatus = ProtectedSessionStatus(0);
/// The header's `D3D12_PROTECTED_SESSION_STATUS_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_protected_session_status).
pub const D3D12_PROTECTED_SESSION_STATUS_INVALID: ProtectedSessionStatus =
    ProtectedSessionStatus(1);

/// The header's `D3D12_QUERY_HEAP_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_heap_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct QueryHeapType(pub i32);

/// The header's `D3D12_QUERY_HEAP_TYPE_OCCLUSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_heap_type).
pub const D3D12_QUERY_HEAP_TYPE_OCCLUSION: QueryHeapType = QueryHeapType(0);
/// The header's `D3D12_QUERY_HEAP_TYPE_TIMESTAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_heap_type).
pub const D3D12_QUERY_HEAP_TYPE_TIMESTAMP: QueryHeapType = QueryHeapType(1);
/// The header's `D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_heap_type).
pub const D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS: QueryHeapType = QueryHeapType(2);
/// The header's `D3D12_QUERY_HEAP_TYPE_SO_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_heap_type).
pub const D3D12_QUERY_HEAP_TYPE_SO_STATISTICS: QueryHeapType = QueryHeapType(3);
/// The header's `D3D12_QUERY_HEAP_TYPE_VIDEO_DECODE_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_heap_type).
pub const D3D12_QUERY_HEAP_TYPE_VIDEO_DECODE_STATISTICS: QueryHeapType = QueryHeapType(4);
/// The header's `D3D12_QUERY_HEAP_TYPE_COPY_QUEUE_TIMESTAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_heap_type).
pub const D3D12_QUERY_HEAP_TYPE_COPY_QUEUE_TIMESTAMP: QueryHeapType = QueryHeapType(5);
/// The header's `D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_heap_type).
pub const D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS1: QueryHeapType = QueryHeapType(7);

/// The header's `D3D12_QUERY_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct QueryType(pub i32);

/// The header's `D3D12_QUERY_TYPE_OCCLUSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_OCCLUSION: QueryType = QueryType(0);
/// The header's `D3D12_QUERY_TYPE_BINARY_OCCLUSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_BINARY_OCCLUSION: QueryType = QueryType(1);
/// The header's `D3D12_QUERY_TYPE_TIMESTAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_TIMESTAMP: QueryType = QueryType(2);
/// The header's `D3D12_QUERY_TYPE_PIPELINE_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_PIPELINE_STATISTICS: QueryType = QueryType(3);
/// The header's `D3D12_QUERY_TYPE_SO_STATISTICS_STREAM0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_SO_STATISTICS_STREAM0: QueryType = QueryType(4);
/// The header's `D3D12_QUERY_TYPE_SO_STATISTICS_STREAM1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_SO_STATISTICS_STREAM1: QueryType = QueryType(5);
/// The header's `D3D12_QUERY_TYPE_SO_STATISTICS_STREAM2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_SO_STATISTICS_STREAM2: QueryType = QueryType(6);
/// The header's `D3D12_QUERY_TYPE_SO_STATISTICS_STREAM3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_SO_STATISTICS_STREAM3: QueryType = QueryType(7);
/// The header's `D3D12_QUERY_TYPE_VIDEO_DECODE_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_VIDEO_DECODE_STATISTICS: QueryType = QueryType(8);
/// The header's `D3D12_QUERY_TYPE_PIPELINE_STATISTICS1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_query_type).
pub const D3D12_QUERY_TYPE_PIPELINE_STATISTICS1: QueryType = QueryType(10);

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_build_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingAccelerationStructureBuildFlags(pub i32);

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_build_flags).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_NONE:
    RaytracingAccelerationStructureBuildFlags = RaytracingAccelerationStructureBuildFlags(0);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_ALLOW_UPDATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_build_flags).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_ALLOW_UPDATE:
    RaytracingAccelerationStructureBuildFlags = RaytracingAccelerationStructureBuildFlags(1);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_ALLOW_COMPACTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_build_flags).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_ALLOW_COMPACTION:
    RaytracingAccelerationStructureBuildFlags = RaytracingAccelerationStructureBuildFlags(2);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PREFER_FAST_TRACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_build_flags).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PREFER_FAST_TRACE:
    RaytracingAccelerationStructureBuildFlags = RaytracingAccelerationStructureBuildFlags(4);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PREFER_FAST_BUILD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_build_flags).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PREFER_FAST_BUILD:
    RaytracingAccelerationStructureBuildFlags = RaytracingAccelerationStructureBuildFlags(8);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_MINIMIZE_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_build_flags).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_MINIMIZE_MEMORY:
    RaytracingAccelerationStructureBuildFlags = RaytracingAccelerationStructureBuildFlags(16);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PERFORM_UPDATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_build_flags).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PERFORM_UPDATE:
    RaytracingAccelerationStructureBuildFlags = RaytracingAccelerationStructureBuildFlags(32);

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_copy_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingAccelerationStructureCopyMode(pub i32);

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_CLONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_copy_mode).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_CLONE:
    RaytracingAccelerationStructureCopyMode = RaytracingAccelerationStructureCopyMode(0);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_COMPACT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_copy_mode).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_COMPACT:
    RaytracingAccelerationStructureCopyMode = RaytracingAccelerationStructureCopyMode(1);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_VISUALIZATION_DECODE_FOR_TOOLS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_copy_mode).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_VISUALIZATION_DECODE_FOR_TOOLS:
    RaytracingAccelerationStructureCopyMode = RaytracingAccelerationStructureCopyMode(2);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_SERIALIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_copy_mode).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_SERIALIZE:
    RaytracingAccelerationStructureCopyMode = RaytracingAccelerationStructureCopyMode(3);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_DESERIALIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_copy_mode).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE_DESERIALIZE:
    RaytracingAccelerationStructureCopyMode = RaytracingAccelerationStructureCopyMode(4);

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingAccelerationStructurePostbuildInfoType(pub i32);

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_COMPACTED_SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_type).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_COMPACTED_SIZE:
    RaytracingAccelerationStructurePostbuildInfoType =
    RaytracingAccelerationStructurePostbuildInfoType(0);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_TOOLS_VISUALIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_type).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_TOOLS_VISUALIZATION:
    RaytracingAccelerationStructurePostbuildInfoType =
    RaytracingAccelerationStructurePostbuildInfoType(1);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_SERIALIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_type).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_SERIALIZATION:
    RaytracingAccelerationStructurePostbuildInfoType =
    RaytracingAccelerationStructurePostbuildInfoType(2);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_CURRENT_SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_type).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_CURRENT_SIZE:
    RaytracingAccelerationStructurePostbuildInfoType =
    RaytracingAccelerationStructurePostbuildInfoType(3);

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingAccelerationStructureType(pub i32);

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_type).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL:
    RaytracingAccelerationStructureType = RaytracingAccelerationStructureType(0);
/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_acceleration_structure_type).
pub const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL:
    RaytracingAccelerationStructureType = RaytracingAccelerationStructureType(1);

/// The header's `D3D12_RAYTRACING_GEOMETRY_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_geometry_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingGeometryFlags(pub i32);

/// The header's `D3D12_RAYTRACING_GEOMETRY_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_geometry_flags).
pub const D3D12_RAYTRACING_GEOMETRY_FLAG_NONE: RaytracingGeometryFlags = RaytracingGeometryFlags(0);
/// The header's `D3D12_RAYTRACING_GEOMETRY_FLAG_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_geometry_flags).
pub const D3D12_RAYTRACING_GEOMETRY_FLAG_OPAQUE: RaytracingGeometryFlags =
    RaytracingGeometryFlags(1);
/// The header's `D3D12_RAYTRACING_GEOMETRY_FLAG_NO_DUPLICATE_ANYHIT_INVOCATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_geometry_flags).
pub const D3D12_RAYTRACING_GEOMETRY_FLAG_NO_DUPLICATE_ANYHIT_INVOCATION: RaytracingGeometryFlags =
    RaytracingGeometryFlags(2);

/// The header's `D3D12_RAYTRACING_GEOMETRY_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_geometry_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingGeometryType(pub i32);

/// The header's `D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_geometry_type).
pub const D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES: RaytracingGeometryType =
    RaytracingGeometryType(0);
/// The header's `D3D12_RAYTRACING_GEOMETRY_TYPE_PROCEDURAL_PRIMITIVE_AABBS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_geometry_type).
pub const D3D12_RAYTRACING_GEOMETRY_TYPE_PROCEDURAL_PRIMITIVE_AABBS: RaytracingGeometryType =
    RaytracingGeometryType(1);

/// The header's `D3D12_RAYTRACING_INSTANCE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_instance_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingInstanceFlags(pub i32);

/// The header's `D3D12_RAYTRACING_INSTANCE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_instance_flags).
pub const D3D12_RAYTRACING_INSTANCE_FLAG_NONE: RaytracingInstanceFlags = RaytracingInstanceFlags(0);
/// The header's `D3D12_RAYTRACING_INSTANCE_FLAG_TRIANGLE_CULL_DISABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_instance_flags).
pub const D3D12_RAYTRACING_INSTANCE_FLAG_TRIANGLE_CULL_DISABLE: RaytracingInstanceFlags =
    RaytracingInstanceFlags(1);
/// The header's `D3D12_RAYTRACING_INSTANCE_FLAG_TRIANGLE_FRONT_COUNTERCLOCKWISE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_instance_flags).
pub const D3D12_RAYTRACING_INSTANCE_FLAG_TRIANGLE_FRONT_COUNTERCLOCKWISE: RaytracingInstanceFlags =
    RaytracingInstanceFlags(2);
/// The header's `D3D12_RAYTRACING_INSTANCE_FLAG_FORCE_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_instance_flags).
pub const D3D12_RAYTRACING_INSTANCE_FLAG_FORCE_OPAQUE: RaytracingInstanceFlags =
    RaytracingInstanceFlags(4);
/// The header's `D3D12_RAYTRACING_INSTANCE_FLAG_FORCE_NON_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_instance_flags).
pub const D3D12_RAYTRACING_INSTANCE_FLAG_FORCE_NON_OPAQUE: RaytracingInstanceFlags =
    RaytracingInstanceFlags(8);

/// The header's `D3D12_RAYTRACING_PIPELINE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_pipeline_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingPipelineFlags(pub i32);

/// The header's `D3D12_RAYTRACING_PIPELINE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_pipeline_flags).
pub const D3D12_RAYTRACING_PIPELINE_FLAG_NONE: RaytracingPipelineFlags = RaytracingPipelineFlags(0);
/// The header's `D3D12_RAYTRACING_PIPELINE_FLAG_SKIP_TRIANGLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_pipeline_flags).
pub const D3D12_RAYTRACING_PIPELINE_FLAG_SKIP_TRIANGLES: RaytracingPipelineFlags =
    RaytracingPipelineFlags(0x100);
/// The header's `D3D12_RAYTRACING_PIPELINE_FLAG_SKIP_PROCEDURAL_PRIMITIVES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_pipeline_flags).
pub const D3D12_RAYTRACING_PIPELINE_FLAG_SKIP_PROCEDURAL_PRIMITIVES: RaytracingPipelineFlags =
    RaytracingPipelineFlags(0x200);

/// The header's `D3D12_RAYTRACING_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaytracingTier(pub i32);

/// The header's `D3D12_RAYTRACING_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_tier).
pub const D3D12_RAYTRACING_TIER_NOT_SUPPORTED: RaytracingTier = RaytracingTier(0);
/// The header's `D3D12_RAYTRACING_TIER_1_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_tier).
pub const D3D12_RAYTRACING_TIER_1_0: RaytracingTier = RaytracingTier(10);
/// The header's `D3D12_RAYTRACING_TIER_1_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_raytracing_tier).
pub const D3D12_RAYTRACING_TIER_1_1: RaytracingTier = RaytracingTier(11);

/// The header's `D3D12_RAY_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RayFlags(pub i32);

/// The header's `D3D12_RAY_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_NONE: RayFlags = RayFlags(0);
/// The header's `D3D12_RAY_FLAG_FORCE_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_FORCE_OPAQUE: RayFlags = RayFlags(1);
/// The header's `D3D12_RAY_FLAG_FORCE_NON_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_FORCE_NON_OPAQUE: RayFlags = RayFlags(2);
/// The header's `D3D12_RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH: RayFlags = RayFlags(4);
/// The header's `D3D12_RAY_FLAG_SKIP_CLOSEST_HIT_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_SKIP_CLOSEST_HIT_SHADER: RayFlags = RayFlags(8);
/// The header's `D3D12_RAY_FLAG_CULL_BACK_FACING_TRIANGLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_CULL_BACK_FACING_TRIANGLES: RayFlags = RayFlags(16);
/// The header's `D3D12_RAY_FLAG_CULL_FRONT_FACING_TRIANGLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_CULL_FRONT_FACING_TRIANGLES: RayFlags = RayFlags(32);
/// The header's `D3D12_RAY_FLAG_CULL_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_CULL_OPAQUE: RayFlags = RayFlags(64);
/// The header's `D3D12_RAY_FLAG_CULL_NON_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_CULL_NON_OPAQUE: RayFlags = RayFlags(128);
/// The header's `D3D12_RAY_FLAG_SKIP_TRIANGLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_SKIP_TRIANGLES: RayFlags = RayFlags(0x100);
/// The header's `D3D12_RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_ray_flags).
pub const D3D12_RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES: RayFlags = RayFlags(0x200);

/// The header's `D3D12_RECREATE_AT_TIER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RecreateAtTier(pub i32);

/// The header's `D3D12_RECREATE_AT_TIER_NOT_SUPPORTED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_RECREATE_AT_TIER_NOT_SUPPORTED: RecreateAtTier = RecreateAtTier(0);
/// The header's `D3D12_RECREATE_AT_TIER_1`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_RECREATE_AT_TIER_1: RecreateAtTier = RecreateAtTier(1);

/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_beginning_access_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RenderPassBeginningAccessType(pub i32);

/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_beginning_access_type).
pub const D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD: RenderPassBeginningAccessType =
    RenderPassBeginningAccessType(0);
/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_beginning_access_type).
pub const D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE: RenderPassBeginningAccessType =
    RenderPassBeginningAccessType(1);
/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_beginning_access_type).
pub const D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR: RenderPassBeginningAccessType =
    RenderPassBeginningAccessType(2);
/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_NO_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_beginning_access_type).
pub const D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_NO_ACCESS: RenderPassBeginningAccessType =
    RenderPassBeginningAccessType(3);
/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE_LOCAL_RENDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_beginning_access_type).
pub const D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE_LOCAL_RENDER:
    RenderPassBeginningAccessType = RenderPassBeginningAccessType(4);
/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE_LOCAL_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_beginning_access_type).
pub const D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE_LOCAL_SRV:
    RenderPassBeginningAccessType = RenderPassBeginningAccessType(5);
/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE_LOCAL_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_beginning_access_type).
pub const D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE_LOCAL_UAV:
    RenderPassBeginningAccessType = RenderPassBeginningAccessType(6);

/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_ending_access_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RenderPassEndingAccessType(pub i32);

/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_DISCARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_ending_access_type).
pub const D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_DISCARD: RenderPassEndingAccessType =
    RenderPassEndingAccessType(0);
/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_ending_access_type).
pub const D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE: RenderPassEndingAccessType =
    RenderPassEndingAccessType(1);
/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_RESOLVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_ending_access_type).
pub const D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_RESOLVE: RenderPassEndingAccessType =
    RenderPassEndingAccessType(2);
/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_NO_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_ending_access_type).
pub const D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_NO_ACCESS: RenderPassEndingAccessType =
    RenderPassEndingAccessType(3);
/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE_LOCAL_RENDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_ending_access_type).
pub const D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE_LOCAL_RENDER: RenderPassEndingAccessType =
    RenderPassEndingAccessType(4);
/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE_LOCAL_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_ending_access_type).
pub const D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE_LOCAL_SRV: RenderPassEndingAccessType =
    RenderPassEndingAccessType(5);
/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE_LOCAL_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_ending_access_type).
pub const D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE_LOCAL_UAV: RenderPassEndingAccessType =
    RenderPassEndingAccessType(6);

/// The header's `D3D12_RENDER_PASS_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RenderPassFlags(pub i32);

/// The header's `D3D12_RENDER_PASS_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_flags).
pub const D3D12_RENDER_PASS_FLAG_NONE: RenderPassFlags = RenderPassFlags(0);
/// The header's `D3D12_RENDER_PASS_FLAG_ALLOW_UAV_WRITES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_flags).
pub const D3D12_RENDER_PASS_FLAG_ALLOW_UAV_WRITES: RenderPassFlags = RenderPassFlags(1);
/// The header's `D3D12_RENDER_PASS_FLAG_SUSPENDING_PASS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_flags).
pub const D3D12_RENDER_PASS_FLAG_SUSPENDING_PASS: RenderPassFlags = RenderPassFlags(2);
/// The header's `D3D12_RENDER_PASS_FLAG_RESUMING_PASS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_flags).
pub const D3D12_RENDER_PASS_FLAG_RESUMING_PASS: RenderPassFlags = RenderPassFlags(4);
/// The header's `D3D12_RENDER_PASS_FLAG_BIND_READ_ONLY_DEPTH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_flags).
pub const D3D12_RENDER_PASS_FLAG_BIND_READ_ONLY_DEPTH: RenderPassFlags = RenderPassFlags(8);
/// The header's `D3D12_RENDER_PASS_FLAG_BIND_READ_ONLY_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_flags).
pub const D3D12_RENDER_PASS_FLAG_BIND_READ_ONLY_STENCIL: RenderPassFlags = RenderPassFlags(16);

/// The header's `D3D12_RENDER_PASS_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RenderPassTier(pub i32);

/// The header's `D3D12_RENDER_PASS_TIER_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_tier).
pub const D3D12_RENDER_PASS_TIER_0: RenderPassTier = RenderPassTier(0);
/// The header's `D3D12_RENDER_PASS_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_tier).
pub const D3D12_RENDER_PASS_TIER_1: RenderPassTier = RenderPassTier(1);
/// The header's `D3D12_RENDER_PASS_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_render_pass_tier).
pub const D3D12_RENDER_PASS_TIER_2: RenderPassTier = RenderPassTier(2);

/// The header's `D3D12_RESIDENCY_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResidencyFlags(pub i32);

/// The header's `D3D12_RESIDENCY_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_flags).
pub const D3D12_RESIDENCY_FLAG_NONE: ResidencyFlags = ResidencyFlags(0);
/// The header's `D3D12_RESIDENCY_FLAG_DENY_OVERBUDGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_flags).
pub const D3D12_RESIDENCY_FLAG_DENY_OVERBUDGET: ResidencyFlags = ResidencyFlags(1);

/// The header's `D3D12_RESIDENCY_PRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_priority).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResidencyPriority(pub i32);

/// The header's `D3D12_RESIDENCY_PRIORITY_MINIMUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_priority).
pub const D3D12_RESIDENCY_PRIORITY_MINIMUM: ResidencyPriority = ResidencyPriority(0x28000000);
/// The header's `D3D12_RESIDENCY_PRIORITY_LOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_priority).
pub const D3D12_RESIDENCY_PRIORITY_LOW: ResidencyPriority = ResidencyPriority(0x50000000);
/// The header's `D3D12_RESIDENCY_PRIORITY_NORMAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_priority).
pub const D3D12_RESIDENCY_PRIORITY_NORMAL: ResidencyPriority = ResidencyPriority(0x78000000);
/// The header's `D3D12_RESIDENCY_PRIORITY_HIGH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_priority).
pub const D3D12_RESIDENCY_PRIORITY_HIGH: ResidencyPriority = ResidencyPriority(-1610547200);
/// The header's `D3D12_RESIDENCY_PRIORITY_MAXIMUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_residency_priority).
pub const D3D12_RESIDENCY_PRIORITY_MAXIMUM: ResidencyPriority = ResidencyPriority(-939524096);

/// The header's `D3D12_RESOLVE_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resolve_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResolveMode(pub i32);

/// The header's `D3D12_RESOLVE_MODE_DECOMPRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resolve_mode).
pub const D3D12_RESOLVE_MODE_DECOMPRESS: ResolveMode = ResolveMode(0);
/// The header's `D3D12_RESOLVE_MODE_MIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resolve_mode).
pub const D3D12_RESOLVE_MODE_MIN: ResolveMode = ResolveMode(1);
/// The header's `D3D12_RESOLVE_MODE_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resolve_mode).
pub const D3D12_RESOLVE_MODE_MAX: ResolveMode = ResolveMode(2);
/// The header's `D3D12_RESOLVE_MODE_AVERAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resolve_mode).
pub const D3D12_RESOLVE_MODE_AVERAGE: ResolveMode = ResolveMode(3);
/// The header's `D3D12_RESOLVE_MODE_ENCODE_SAMPLER_FEEDBACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resolve_mode).
pub const D3D12_RESOLVE_MODE_ENCODE_SAMPLER_FEEDBACK: ResolveMode = ResolveMode(4);
/// The header's `D3D12_RESOLVE_MODE_DECODE_SAMPLER_FEEDBACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resolve_mode).
pub const D3D12_RESOLVE_MODE_DECODE_SAMPLER_FEEDBACK: ResolveMode = ResolveMode(5);

/// The header's `D3D12_RESOURCE_BARRIER_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_barrier_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceBarrierFlags(pub i32);

/// The header's `D3D12_RESOURCE_BARRIER_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_barrier_flags).
pub const D3D12_RESOURCE_BARRIER_FLAG_NONE: ResourceBarrierFlags = ResourceBarrierFlags(0);
/// The header's `D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_barrier_flags).
pub const D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY: ResourceBarrierFlags = ResourceBarrierFlags(1);
/// The header's `D3D12_RESOURCE_BARRIER_FLAG_END_ONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_barrier_flags).
pub const D3D12_RESOURCE_BARRIER_FLAG_END_ONLY: ResourceBarrierFlags = ResourceBarrierFlags(2);

/// The header's `D3D12_RESOURCE_BARRIER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_barrier_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceBarrierType(pub i32);

/// The header's `D3D12_RESOURCE_BARRIER_TYPE_TRANSITION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_barrier_type).
pub const D3D12_RESOURCE_BARRIER_TYPE_TRANSITION: ResourceBarrierType = ResourceBarrierType(0);
/// The header's `D3D12_RESOURCE_BARRIER_TYPE_ALIASING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_barrier_type).
pub const D3D12_RESOURCE_BARRIER_TYPE_ALIASING: ResourceBarrierType = ResourceBarrierType(1);
/// The header's `D3D12_RESOURCE_BARRIER_TYPE_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_barrier_type).
pub const D3D12_RESOURCE_BARRIER_TYPE_UAV: ResourceBarrierType = ResourceBarrierType(2);

/// The header's `D3D12_RESOURCE_BINDING_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_binding_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceBindingTier(pub i32);

/// The header's `D3D12_RESOURCE_BINDING_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_binding_tier).
pub const D3D12_RESOURCE_BINDING_TIER_1: ResourceBindingTier = ResourceBindingTier(1);
/// The header's `D3D12_RESOURCE_BINDING_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_binding_tier).
pub const D3D12_RESOURCE_BINDING_TIER_2: ResourceBindingTier = ResourceBindingTier(2);
/// The header's `D3D12_RESOURCE_BINDING_TIER_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_binding_tier).
pub const D3D12_RESOURCE_BINDING_TIER_3: ResourceBindingTier = ResourceBindingTier(3);

/// The header's `D3D12_RESOURCE_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceDimension(pub i32);

/// The header's `D3D12_RESOURCE_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_dimension).
pub const D3D12_RESOURCE_DIMENSION_UNKNOWN: ResourceDimension = ResourceDimension(0);
/// The header's `D3D12_RESOURCE_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_dimension).
pub const D3D12_RESOURCE_DIMENSION_BUFFER: ResourceDimension = ResourceDimension(1);
/// The header's `D3D12_RESOURCE_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_dimension).
pub const D3D12_RESOURCE_DIMENSION_TEXTURE1D: ResourceDimension = ResourceDimension(2);
/// The header's `D3D12_RESOURCE_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_dimension).
pub const D3D12_RESOURCE_DIMENSION_TEXTURE2D: ResourceDimension = ResourceDimension(3);
/// The header's `D3D12_RESOURCE_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_dimension).
pub const D3D12_RESOURCE_DIMENSION_TEXTURE3D: ResourceDimension = ResourceDimension(4);

/// The header's `D3D12_RESOURCE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceFlags(pub i32);

/// The header's `D3D12_RESOURCE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_NONE: ResourceFlags = ResourceFlags(0);
/// The header's `D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET: ResourceFlags = ResourceFlags(1);
/// The header's `D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL: ResourceFlags = ResourceFlags(2);
/// The header's `D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS: ResourceFlags = ResourceFlags(4);
/// The header's `D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE: ResourceFlags = ResourceFlags(8);
/// The header's `D3D12_RESOURCE_FLAG_ALLOW_CROSS_ADAPTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_ALLOW_CROSS_ADAPTER: ResourceFlags = ResourceFlags(16);
/// The header's `D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS: ResourceFlags = ResourceFlags(32);
/// The header's `D3D12_RESOURCE_FLAG_VIDEO_DECODE_REFERENCE_ONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_VIDEO_DECODE_REFERENCE_ONLY: ResourceFlags = ResourceFlags(64);
/// The header's `D3D12_RESOURCE_FLAG_VIDEO_ENCODE_REFERENCE_ONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_VIDEO_ENCODE_REFERENCE_ONLY: ResourceFlags = ResourceFlags(128);
/// The header's `D3D12_RESOURCE_FLAG_RAYTRACING_ACCELERATION_STRUCTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_flags).
pub const D3D12_RESOURCE_FLAG_RAYTRACING_ACCELERATION_STRUCTURE: ResourceFlags =
    ResourceFlags(0x100);

/// The header's `D3D12_RESOURCE_HEAP_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_heap_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceHeapTier(pub i32);

/// The header's `D3D12_RESOURCE_HEAP_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_heap_tier).
pub const D3D12_RESOURCE_HEAP_TIER_1: ResourceHeapTier = ResourceHeapTier(1);
/// The header's `D3D12_RESOURCE_HEAP_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_heap_tier).
pub const D3D12_RESOURCE_HEAP_TIER_2: ResourceHeapTier = ResourceHeapTier(2);

/// The header's `D3D12_RESOURCE_STATES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceStates(pub i32);

/// The header's `D3D12_RESOURCE_STATE_COMMON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_COMMON: ResourceStates = ResourceStates(0);
/// The header's `D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER: ResourceStates = ResourceStates(1);
/// The header's `D3D12_RESOURCE_STATE_INDEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_INDEX_BUFFER: ResourceStates = ResourceStates(2);
/// The header's `D3D12_RESOURCE_STATE_RENDER_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RENDER_TARGET: ResourceStates = ResourceStates(4);
/// The header's `D3D12_RESOURCE_STATE_UNORDERED_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_UNORDERED_ACCESS: ResourceStates = ResourceStates(8);
/// The header's `D3D12_RESOURCE_STATE_DEPTH_WRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_DEPTH_WRITE: ResourceStates = ResourceStates(16);
/// The header's `D3D12_RESOURCE_STATE_DEPTH_READ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_DEPTH_READ: ResourceStates = ResourceStates(32);
/// The header's `D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE: ResourceStates = ResourceStates(64);
/// The header's `D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE: ResourceStates = ResourceStates(128);
/// The header's `D3D12_RESOURCE_STATE_STREAM_OUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_STREAM_OUT: ResourceStates = ResourceStates(0x100);
/// The header's `D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT: ResourceStates = ResourceStates(0x200);
/// The header's `D3D12_RESOURCE_STATE_COPY_DEST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_COPY_DEST: ResourceStates = ResourceStates(0x400);
/// The header's `D3D12_RESOURCE_STATE_COPY_SOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_COPY_SOURCE: ResourceStates = ResourceStates(0x800);
/// The header's `D3D12_RESOURCE_STATE_RESOLVE_DEST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RESOLVE_DEST: ResourceStates = ResourceStates(0x1000);
/// The header's `D3D12_RESOURCE_STATE_RESOLVE_SOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RESOLVE_SOURCE: ResourceStates = ResourceStates(0x2000);
/// The header's `D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE: ResourceStates =
    ResourceStates(0x400000);
/// The header's `D3D12_RESOURCE_STATE_SHADING_RATE_SOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_SHADING_RATE_SOURCE: ResourceStates = ResourceStates(0x1000000);
/// The header's `D3D12_RESOURCE_STATE_RESERVED_INTERNAL_8000`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RESERVED_INTERNAL_8000: ResourceStates = ResourceStates(0x8000);
/// The header's `D3D12_RESOURCE_STATE_RESERVED_INTERNAL_4000`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RESERVED_INTERNAL_4000: ResourceStates = ResourceStates(0x4000);
/// The header's `D3D12_RESOURCE_STATE_RESERVED_INTERNAL_100000`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RESERVED_INTERNAL_100000: ResourceStates = ResourceStates(0x100000);
/// The header's `D3D12_RESOURCE_STATE_RESERVED_INTERNAL_40000000`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RESERVED_INTERNAL_40000000: ResourceStates =
    ResourceStates(0x40000000);
/// The header's `D3D12_RESOURCE_STATE_RESERVED_INTERNAL_80000000`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_RESERVED_INTERNAL_80000000: ResourceStates =
    ResourceStates(-2147483648);
/// The header's `D3D12_RESOURCE_STATE_GENERIC_READ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_GENERIC_READ: ResourceStates = ResourceStates(0xac3);
/// The header's `D3D12_RESOURCE_STATE_ALL_SHADER_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_ALL_SHADER_RESOURCE: ResourceStates = ResourceStates(192);
/// The header's `D3D12_RESOURCE_STATE_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_PRESENT: ResourceStates = ResourceStates(0);
/// The header's `D3D12_RESOURCE_STATE_PREDICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_PREDICATION: ResourceStates = ResourceStates(0x200);
/// The header's `D3D12_RESOURCE_STATE_VIDEO_DECODE_READ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_VIDEO_DECODE_READ: ResourceStates = ResourceStates(0x10000);
/// The header's `D3D12_RESOURCE_STATE_VIDEO_DECODE_WRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_VIDEO_DECODE_WRITE: ResourceStates = ResourceStates(0x20000);
/// The header's `D3D12_RESOURCE_STATE_VIDEO_PROCESS_READ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_VIDEO_PROCESS_READ: ResourceStates = ResourceStates(0x40000);
/// The header's `D3D12_RESOURCE_STATE_VIDEO_PROCESS_WRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_VIDEO_PROCESS_WRITE: ResourceStates = ResourceStates(0x80000);
/// The header's `D3D12_RESOURCE_STATE_VIDEO_ENCODE_READ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_VIDEO_ENCODE_READ: ResourceStates = ResourceStates(0x200000);
/// The header's `D3D12_RESOURCE_STATE_VIDEO_ENCODE_WRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_resource_states).
pub const D3D12_RESOURCE_STATE_VIDEO_ENCODE_WRITE: ResourceStates = ResourceStates(0x800000);

/// The header's `D3D12_RLDO_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_rldo_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RldoFlags(pub i32);

/// The header's `D3D12_RLDO_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_rldo_flags).
pub const D3D12_RLDO_NONE: RldoFlags = RldoFlags(0);
/// The header's `D3D12_RLDO_SUMMARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_rldo_flags).
pub const D3D12_RLDO_SUMMARY: RldoFlags = RldoFlags(1);
/// The header's `D3D12_RLDO_DETAIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_rldo_flags).
pub const D3D12_RLDO_DETAIL: RldoFlags = RldoFlags(2);
/// The header's `D3D12_RLDO_IGNORE_INTERNAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ne-d3d12sdklayers-d3d12_rldo_flags).
pub const D3D12_RLDO_IGNORE_INTERNAL: RldoFlags = RldoFlags(4);

/// The header's `D3D12_ROOT_DESCRIPTOR_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_descriptor_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RootDescriptorFlags(pub i32);

/// The header's `D3D12_ROOT_DESCRIPTOR_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_descriptor_flags).
pub const D3D12_ROOT_DESCRIPTOR_FLAG_NONE: RootDescriptorFlags = RootDescriptorFlags(0);
/// The header's `D3D12_ROOT_DESCRIPTOR_FLAG_DATA_VOLATILE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_descriptor_flags).
pub const D3D12_ROOT_DESCRIPTOR_FLAG_DATA_VOLATILE: RootDescriptorFlags = RootDescriptorFlags(2);
/// The header's `D3D12_ROOT_DESCRIPTOR_FLAG_DATA_STATIC_WHILE_SET_AT_EXECUTE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_descriptor_flags).
pub const D3D12_ROOT_DESCRIPTOR_FLAG_DATA_STATIC_WHILE_SET_AT_EXECUTE: RootDescriptorFlags =
    RootDescriptorFlags(4);
/// The header's `D3D12_ROOT_DESCRIPTOR_FLAG_DATA_STATIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_descriptor_flags).
pub const D3D12_ROOT_DESCRIPTOR_FLAG_DATA_STATIC: RootDescriptorFlags = RootDescriptorFlags(8);

/// The header's `D3D12_ROOT_PARAMETER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_parameter_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RootParameterType(pub i32);

/// The header's `D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_parameter_type).
pub const D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE: RootParameterType = RootParameterType(0);
/// The header's `D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_parameter_type).
pub const D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS: RootParameterType = RootParameterType(1);
/// The header's `D3D12_ROOT_PARAMETER_TYPE_CBV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_parameter_type).
pub const D3D12_ROOT_PARAMETER_TYPE_CBV: RootParameterType = RootParameterType(2);
/// The header's `D3D12_ROOT_PARAMETER_TYPE_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_parameter_type).
pub const D3D12_ROOT_PARAMETER_TYPE_SRV: RootParameterType = RootParameterType(3);
/// The header's `D3D12_ROOT_PARAMETER_TYPE_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_parameter_type).
pub const D3D12_ROOT_PARAMETER_TYPE_UAV: RootParameterType = RootParameterType(4);

/// The header's `D3D12_ROOT_SIGNATURE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RootSignatureFlags(pub i32);

/// The header's `D3D12_ROOT_SIGNATURE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_NONE: RootSignatureFlags = RootSignatureFlags(0);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT: RootSignatureFlags =
    RootSignatureFlags(1);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_DENY_VERTEX_SHADER_ROOT_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_DENY_VERTEX_SHADER_ROOT_ACCESS: RootSignatureFlags =
    RootSignatureFlags(2);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_DENY_HULL_SHADER_ROOT_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_DENY_HULL_SHADER_ROOT_ACCESS: RootSignatureFlags =
    RootSignatureFlags(4);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_DENY_DOMAIN_SHADER_ROOT_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_DENY_DOMAIN_SHADER_ROOT_ACCESS: RootSignatureFlags =
    RootSignatureFlags(8);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_DENY_GEOMETRY_SHADER_ROOT_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_DENY_GEOMETRY_SHADER_ROOT_ACCESS: RootSignatureFlags =
    RootSignatureFlags(16);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_DENY_PIXEL_SHADER_ROOT_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_DENY_PIXEL_SHADER_ROOT_ACCESS: RootSignatureFlags =
    RootSignatureFlags(32);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_ALLOW_STREAM_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_ALLOW_STREAM_OUTPUT: RootSignatureFlags =
    RootSignatureFlags(64);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_LOCAL_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_LOCAL_ROOT_SIGNATURE: RootSignatureFlags =
    RootSignatureFlags(128);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_DENY_AMPLIFICATION_SHADER_ROOT_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_DENY_AMPLIFICATION_SHADER_ROOT_ACCESS: RootSignatureFlags =
    RootSignatureFlags(0x100);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_DENY_MESH_SHADER_ROOT_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_DENY_MESH_SHADER_ROOT_ACCESS: RootSignatureFlags =
    RootSignatureFlags(0x200);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_CBV_SRV_UAV_HEAP_DIRECTLY_INDEXED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_CBV_SRV_UAV_HEAP_DIRECTLY_INDEXED: RootSignatureFlags =
    RootSignatureFlags(0x400);
/// The header's `D3D12_ROOT_SIGNATURE_FLAG_SAMPLER_HEAP_DIRECTLY_INDEXED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_root_signature_flags).
pub const D3D12_ROOT_SIGNATURE_FLAG_SAMPLER_HEAP_DIRECTLY_INDEXED: RootSignatureFlags =
    RootSignatureFlags(0x800);

/// The header's `D3D12_RTV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RtvDimension(pub i32);

/// The header's `D3D12_RTV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_UNKNOWN: RtvDimension = RtvDimension(0);
/// The header's `D3D12_RTV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_BUFFER: RtvDimension = RtvDimension(1);
/// The header's `D3D12_RTV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_TEXTURE1D: RtvDimension = RtvDimension(2);
/// The header's `D3D12_RTV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_TEXTURE1DARRAY: RtvDimension = RtvDimension(3);
/// The header's `D3D12_RTV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_TEXTURE2D: RtvDimension = RtvDimension(4);
/// The header's `D3D12_RTV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_TEXTURE2DARRAY: RtvDimension = RtvDimension(5);
/// The header's `D3D12_RTV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_TEXTURE2DMS: RtvDimension = RtvDimension(6);
/// The header's `D3D12_RTV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_TEXTURE2DMSARRAY: RtvDimension = RtvDimension(7);
/// The header's `D3D12_RTV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_rtv_dimension).
pub const D3D12_RTV_DIMENSION_TEXTURE3D: RtvDimension = RtvDimension(8);

/// The header's `D3D12_SAMPLER_FEEDBACK_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_sampler_feedback_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SamplerFeedbackTier(pub i32);

/// The header's `D3D12_SAMPLER_FEEDBACK_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_sampler_feedback_tier).
pub const D3D12_SAMPLER_FEEDBACK_TIER_NOT_SUPPORTED: SamplerFeedbackTier = SamplerFeedbackTier(0);
/// The header's `D3D12_SAMPLER_FEEDBACK_TIER_0_9`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_sampler_feedback_tier).
pub const D3D12_SAMPLER_FEEDBACK_TIER_0_9: SamplerFeedbackTier = SamplerFeedbackTier(90);
/// The header's `D3D12_SAMPLER_FEEDBACK_TIER_1_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_sampler_feedback_tier).
pub const D3D12_SAMPLER_FEEDBACK_TIER_1_0: SamplerFeedbackTier = SamplerFeedbackTier(100);

/// The header's `D3D12_SAMPLER_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SamplerFlags(pub i32);

/// The header's `D3D12_SAMPLER_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_SAMPLER_FLAG_NONE: SamplerFlags = SamplerFlags(0);
/// The header's `D3D12_SAMPLER_FLAG_UINT_BORDER_COLOR`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_SAMPLER_FLAG_UINT_BORDER_COLOR: SamplerFlags = SamplerFlags(1);
/// The header's `D3D12_SAMPLER_FLAG_NON_NORMALIZED_COORDINATES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_SAMPLER_FLAG_NON_NORMALIZED_COORDINATES: SamplerFlags = SamplerFlags(2);

/// The header's `D3D12_SERIALIZED_DATA_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_serialized_data_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SerializedDataType(pub i32);

/// The header's `D3D12_SERIALIZED_DATA_RAYTRACING_ACCELERATION_STRUCTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_serialized_data_type).
pub const D3D12_SERIALIZED_DATA_RAYTRACING_ACCELERATION_STRUCTURE: SerializedDataType =
    SerializedDataType(0);

/// The header's `D3D12_SET_WORK_GRAPH_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SetWorkGraphFlags(pub i32);

/// The header's `D3D12_SET_WORK_GRAPH_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_SET_WORK_GRAPH_FLAG_NONE: SetWorkGraphFlags = SetWorkGraphFlags(0);
/// The header's `D3D12_SET_WORK_GRAPH_FLAG_INITIALIZE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_SET_WORK_GRAPH_FLAG_INITIALIZE: SetWorkGraphFlags = SetWorkGraphFlags(1);

/// The header's `D3D12_SHADER_CACHE_CONTROL_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_control_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderCacheControlFlags(pub i32);

/// The header's `D3D12_SHADER_CACHE_CONTROL_FLAG_DISABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_control_flags).
pub const D3D12_SHADER_CACHE_CONTROL_FLAG_DISABLE: ShaderCacheControlFlags =
    ShaderCacheControlFlags(1);
/// The header's `D3D12_SHADER_CACHE_CONTROL_FLAG_ENABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_control_flags).
pub const D3D12_SHADER_CACHE_CONTROL_FLAG_ENABLE: ShaderCacheControlFlags =
    ShaderCacheControlFlags(2);
/// The header's `D3D12_SHADER_CACHE_CONTROL_FLAG_CLEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_control_flags).
pub const D3D12_SHADER_CACHE_CONTROL_FLAG_CLEAR: ShaderCacheControlFlags =
    ShaderCacheControlFlags(4);

/// The header's `D3D12_SHADER_CACHE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderCacheFlags(pub i32);

/// The header's `D3D12_SHADER_CACHE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_flags).
pub const D3D12_SHADER_CACHE_FLAG_NONE: ShaderCacheFlags = ShaderCacheFlags(0);
/// The header's `D3D12_SHADER_CACHE_FLAG_DRIVER_VERSIONED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_flags).
pub const D3D12_SHADER_CACHE_FLAG_DRIVER_VERSIONED: ShaderCacheFlags = ShaderCacheFlags(1);
/// The header's `D3D12_SHADER_CACHE_FLAG_USE_WORKING_DIR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_flags).
pub const D3D12_SHADER_CACHE_FLAG_USE_WORKING_DIR: ShaderCacheFlags = ShaderCacheFlags(2);

/// The header's `D3D12_SHADER_CACHE_KIND_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_kind_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderCacheKindFlags(pub i32);

/// The header's `D3D12_SHADER_CACHE_KIND_FLAG_IMPLICIT_D3D_CACHE_FOR_DRIVER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_kind_flags).
pub const D3D12_SHADER_CACHE_KIND_FLAG_IMPLICIT_D3D_CACHE_FOR_DRIVER: ShaderCacheKindFlags =
    ShaderCacheKindFlags(1);
/// The header's `D3D12_SHADER_CACHE_KIND_FLAG_IMPLICIT_D3D_CONVERSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_kind_flags).
pub const D3D12_SHADER_CACHE_KIND_FLAG_IMPLICIT_D3D_CONVERSIONS: ShaderCacheKindFlags =
    ShaderCacheKindFlags(2);
/// The header's `D3D12_SHADER_CACHE_KIND_FLAG_IMPLICIT_DRIVER_MANAGED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_kind_flags).
pub const D3D12_SHADER_CACHE_KIND_FLAG_IMPLICIT_DRIVER_MANAGED: ShaderCacheKindFlags =
    ShaderCacheKindFlags(4);
/// The header's `D3D12_SHADER_CACHE_KIND_FLAG_APPLICATION_MANAGED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_kind_flags).
pub const D3D12_SHADER_CACHE_KIND_FLAG_APPLICATION_MANAGED: ShaderCacheKindFlags =
    ShaderCacheKindFlags(8);

/// The header's `D3D12_SHADER_CACHE_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderCacheMode(pub i32);

/// The header's `D3D12_SHADER_CACHE_MODE_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_mode).
pub const D3D12_SHADER_CACHE_MODE_MEMORY: ShaderCacheMode = ShaderCacheMode(0);
/// The header's `D3D12_SHADER_CACHE_MODE_DISK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_mode).
pub const D3D12_SHADER_CACHE_MODE_DISK: ShaderCacheMode = ShaderCacheMode(1);

/// The header's `D3D12_SHADER_CACHE_SUPPORT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderCacheSupportFlags(pub i32);

/// The header's `D3D12_SHADER_CACHE_SUPPORT_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
pub const D3D12_SHADER_CACHE_SUPPORT_NONE: ShaderCacheSupportFlags = ShaderCacheSupportFlags(0);
/// The header's `D3D12_SHADER_CACHE_SUPPORT_SINGLE_PSO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
pub const D3D12_SHADER_CACHE_SUPPORT_SINGLE_PSO: ShaderCacheSupportFlags =
    ShaderCacheSupportFlags(1);
/// The header's `D3D12_SHADER_CACHE_SUPPORT_LIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
pub const D3D12_SHADER_CACHE_SUPPORT_LIBRARY: ShaderCacheSupportFlags = ShaderCacheSupportFlags(2);
/// The header's `D3D12_SHADER_CACHE_SUPPORT_AUTOMATIC_INPROC_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
pub const D3D12_SHADER_CACHE_SUPPORT_AUTOMATIC_INPROC_CACHE: ShaderCacheSupportFlags =
    ShaderCacheSupportFlags(4);
/// The header's `D3D12_SHADER_CACHE_SUPPORT_AUTOMATIC_DISK_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
pub const D3D12_SHADER_CACHE_SUPPORT_AUTOMATIC_DISK_CACHE: ShaderCacheSupportFlags =
    ShaderCacheSupportFlags(8);
/// The header's `D3D12_SHADER_CACHE_SUPPORT_DRIVER_MANAGED_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
pub const D3D12_SHADER_CACHE_SUPPORT_DRIVER_MANAGED_CACHE: ShaderCacheSupportFlags =
    ShaderCacheSupportFlags(16);
/// The header's `D3D12_SHADER_CACHE_SUPPORT_SHADER_CONTROL_CLEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
pub const D3D12_SHADER_CACHE_SUPPORT_SHADER_CONTROL_CLEAR: ShaderCacheSupportFlags =
    ShaderCacheSupportFlags(32);
/// The header's `D3D12_SHADER_CACHE_SUPPORT_SHADER_SESSION_DELETE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_cache_support_flags).
pub const D3D12_SHADER_CACHE_SUPPORT_SHADER_SESSION_DELETE: ShaderCacheSupportFlags =
    ShaderCacheSupportFlags(64);

/// The header's `D3D12_SHADER_COMPONENT_MAPPING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_component_mapping).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderComponentMapping(pub i32);

/// The header's `D3D12_SHADER_COMPONENT_MAPPING_FROM_MEMORY_COMPONENT_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_component_mapping).
pub const D3D12_SHADER_COMPONENT_MAPPING_FROM_MEMORY_COMPONENT_0: ShaderComponentMapping =
    ShaderComponentMapping(0);
/// The header's `D3D12_SHADER_COMPONENT_MAPPING_FROM_MEMORY_COMPONENT_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_component_mapping).
pub const D3D12_SHADER_COMPONENT_MAPPING_FROM_MEMORY_COMPONENT_1: ShaderComponentMapping =
    ShaderComponentMapping(1);
/// The header's `D3D12_SHADER_COMPONENT_MAPPING_FROM_MEMORY_COMPONENT_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_component_mapping).
pub const D3D12_SHADER_COMPONENT_MAPPING_FROM_MEMORY_COMPONENT_2: ShaderComponentMapping =
    ShaderComponentMapping(2);
/// The header's `D3D12_SHADER_COMPONENT_MAPPING_FROM_MEMORY_COMPONENT_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_component_mapping).
pub const D3D12_SHADER_COMPONENT_MAPPING_FROM_MEMORY_COMPONENT_3: ShaderComponentMapping =
    ShaderComponentMapping(3);
/// The header's `D3D12_SHADER_COMPONENT_MAPPING_FORCE_VALUE_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_component_mapping).
pub const D3D12_SHADER_COMPONENT_MAPPING_FORCE_VALUE_0: ShaderComponentMapping =
    ShaderComponentMapping(4);
/// The header's `D3D12_SHADER_COMPONENT_MAPPING_FORCE_VALUE_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_component_mapping).
pub const D3D12_SHADER_COMPONENT_MAPPING_FORCE_VALUE_1: ShaderComponentMapping =
    ShaderComponentMapping(5);

/// The header's `D3D12_SHADER_MIN_PRECISION_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_min_precision_support).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderMinPrecisionSupport(pub i32);

/// The header's `D3D12_SHADER_MIN_PRECISION_SUPPORT_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_min_precision_support).
pub const D3D12_SHADER_MIN_PRECISION_SUPPORT_NONE: ShaderMinPrecisionSupport =
    ShaderMinPrecisionSupport(0);
/// The header's `D3D12_SHADER_MIN_PRECISION_SUPPORT_10_BIT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_min_precision_support).
pub const D3D12_SHADER_MIN_PRECISION_SUPPORT_10_BIT: ShaderMinPrecisionSupport =
    ShaderMinPrecisionSupport(1);
/// The header's `D3D12_SHADER_MIN_PRECISION_SUPPORT_16_BIT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_min_precision_support).
pub const D3D12_SHADER_MIN_PRECISION_SUPPORT_16_BIT: ShaderMinPrecisionSupport =
    ShaderMinPrecisionSupport(2);

/// The header's `D3D12_SHADER_VERSION_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderVersionType(pub i32);

/// The header's `D3D12_SHVER_PIXEL_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_PIXEL_SHADER: ShaderVersionType = ShaderVersionType(0);
/// The header's `D3D12_SHVER_VERTEX_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_VERTEX_SHADER: ShaderVersionType = ShaderVersionType(1);
/// The header's `D3D12_SHVER_GEOMETRY_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_GEOMETRY_SHADER: ShaderVersionType = ShaderVersionType(2);
/// The header's `D3D12_SHVER_HULL_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_HULL_SHADER: ShaderVersionType = ShaderVersionType(3);
/// The header's `D3D12_SHVER_DOMAIN_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_DOMAIN_SHADER: ShaderVersionType = ShaderVersionType(4);
/// The header's `D3D12_SHVER_COMPUTE_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_COMPUTE_SHADER: ShaderVersionType = ShaderVersionType(5);
/// The header's `D3D12_SHVER_LIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_LIBRARY: ShaderVersionType = ShaderVersionType(6);
/// The header's `D3D12_SHVER_RAY_GENERATION_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_RAY_GENERATION_SHADER: ShaderVersionType = ShaderVersionType(7);
/// The header's `D3D12_SHVER_INTERSECTION_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_INTERSECTION_SHADER: ShaderVersionType = ShaderVersionType(8);
/// The header's `D3D12_SHVER_ANY_HIT_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_ANY_HIT_SHADER: ShaderVersionType = ShaderVersionType(9);
/// The header's `D3D12_SHVER_CLOSEST_HIT_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_CLOSEST_HIT_SHADER: ShaderVersionType = ShaderVersionType(10);
/// The header's `D3D12_SHVER_MISS_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_MISS_SHADER: ShaderVersionType = ShaderVersionType(11);
/// The header's `D3D12_SHVER_CALLABLE_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_CALLABLE_SHADER: ShaderVersionType = ShaderVersionType(12);
/// The header's `D3D12_SHVER_MESH_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_MESH_SHADER: ShaderVersionType = ShaderVersionType(13);
/// The header's `D3D12_SHVER_AMPLIFICATION_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_AMPLIFICATION_SHADER: ShaderVersionType = ShaderVersionType(14);
/// The header's `D3D12_SHVER_NODE_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_NODE_SHADER: ShaderVersionType = ShaderVersionType(15);
/// The header's `D3D12_SHVER_RESERVED0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ne-d3d12shader-d3d12_shader_version_type).
pub const D3D12_SHVER_RESERVED0: ShaderVersionType = ShaderVersionType(0xfff0);

/// The header's `D3D12_SHADER_VISIBILITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderVisibility(pub i32);

/// The header's `D3D12_SHADER_VISIBILITY_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
pub const D3D12_SHADER_VISIBILITY_ALL: ShaderVisibility = ShaderVisibility(0);
/// The header's `D3D12_SHADER_VISIBILITY_VERTEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
pub const D3D12_SHADER_VISIBILITY_VERTEX: ShaderVisibility = ShaderVisibility(1);
/// The header's `D3D12_SHADER_VISIBILITY_HULL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
pub const D3D12_SHADER_VISIBILITY_HULL: ShaderVisibility = ShaderVisibility(2);
/// The header's `D3D12_SHADER_VISIBILITY_DOMAIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
pub const D3D12_SHADER_VISIBILITY_DOMAIN: ShaderVisibility = ShaderVisibility(3);
/// The header's `D3D12_SHADER_VISIBILITY_GEOMETRY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
pub const D3D12_SHADER_VISIBILITY_GEOMETRY: ShaderVisibility = ShaderVisibility(4);
/// The header's `D3D12_SHADER_VISIBILITY_PIXEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
pub const D3D12_SHADER_VISIBILITY_PIXEL: ShaderVisibility = ShaderVisibility(5);
/// The header's `D3D12_SHADER_VISIBILITY_AMPLIFICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
pub const D3D12_SHADER_VISIBILITY_AMPLIFICATION: ShaderVisibility = ShaderVisibility(6);
/// The header's `D3D12_SHADER_VISIBILITY_MESH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility).
pub const D3D12_SHADER_VISIBILITY_MESH: ShaderVisibility = ShaderVisibility(7);

/// The header's `D3D12_SHADING_RATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShadingRate(pub i32);

/// The header's `D3D12_SHADING_RATE_1X1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate).
pub const D3D12_SHADING_RATE_1X1: ShadingRate = ShadingRate(0);
/// The header's `D3D12_SHADING_RATE_1X2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate).
pub const D3D12_SHADING_RATE_1X2: ShadingRate = ShadingRate(1);
/// The header's `D3D12_SHADING_RATE_2X1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate).
pub const D3D12_SHADING_RATE_2X1: ShadingRate = ShadingRate(4);
/// The header's `D3D12_SHADING_RATE_2X2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate).
pub const D3D12_SHADING_RATE_2X2: ShadingRate = ShadingRate(5);
/// The header's `D3D12_SHADING_RATE_2X4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate).
pub const D3D12_SHADING_RATE_2X4: ShadingRate = ShadingRate(6);
/// The header's `D3D12_SHADING_RATE_4X2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate).
pub const D3D12_SHADING_RATE_4X2: ShadingRate = ShadingRate(9);
/// The header's `D3D12_SHADING_RATE_4X4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate).
pub const D3D12_SHADING_RATE_4X4: ShadingRate = ShadingRate(10);

/// The header's `D3D12_SHADING_RATE_COMBINER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate_combiner).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShadingRateCombiner(pub i32);

/// The header's `D3D12_SHADING_RATE_COMBINER_PASSTHROUGH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate_combiner).
pub const D3D12_SHADING_RATE_COMBINER_PASSTHROUGH: ShadingRateCombiner = ShadingRateCombiner(0);
/// The header's `D3D12_SHADING_RATE_COMBINER_OVERRIDE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate_combiner).
pub const D3D12_SHADING_RATE_COMBINER_OVERRIDE: ShadingRateCombiner = ShadingRateCombiner(1);
/// The header's `D3D12_SHADING_RATE_COMBINER_MIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate_combiner).
pub const D3D12_SHADING_RATE_COMBINER_MIN: ShadingRateCombiner = ShadingRateCombiner(2);
/// The header's `D3D12_SHADING_RATE_COMBINER_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate_combiner).
pub const D3D12_SHADING_RATE_COMBINER_MAX: ShadingRateCombiner = ShadingRateCombiner(3);
/// The header's `D3D12_SHADING_RATE_COMBINER_SUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shading_rate_combiner).
pub const D3D12_SHADING_RATE_COMBINER_SUM: ShadingRateCombiner = ShadingRateCombiner(4);

/// The header's `D3D12_SHARED_RESOURCE_COMPATIBILITY_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shared_resource_compatibility_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SharedResourceCompatibilityTier(pub i32);

/// The header's `D3D12_SHARED_RESOURCE_COMPATIBILITY_TIER_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shared_resource_compatibility_tier).
pub const D3D12_SHARED_RESOURCE_COMPATIBILITY_TIER_0: SharedResourceCompatibilityTier =
    SharedResourceCompatibilityTier(0);
/// The header's `D3D12_SHARED_RESOURCE_COMPATIBILITY_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shared_resource_compatibility_tier).
pub const D3D12_SHARED_RESOURCE_COMPATIBILITY_TIER_1: SharedResourceCompatibilityTier =
    SharedResourceCompatibilityTier(1);
/// The header's `D3D12_SHARED_RESOURCE_COMPATIBILITY_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_shared_resource_compatibility_tier).
pub const D3D12_SHARED_RESOURCE_COMPATIBILITY_TIER_2: SharedResourceCompatibilityTier =
    SharedResourceCompatibilityTier(2);

/// The header's `D3D12_SRV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SrvDimension(pub i32);

/// The header's `D3D12_SRV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_UNKNOWN: SrvDimension = SrvDimension(0);
/// The header's `D3D12_SRV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_BUFFER: SrvDimension = SrvDimension(1);
/// The header's `D3D12_SRV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURE1D: SrvDimension = SrvDimension(2);
/// The header's `D3D12_SRV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURE1DARRAY: SrvDimension = SrvDimension(3);
/// The header's `D3D12_SRV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURE2D: SrvDimension = SrvDimension(4);
/// The header's `D3D12_SRV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURE2DARRAY: SrvDimension = SrvDimension(5);
/// The header's `D3D12_SRV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURE2DMS: SrvDimension = SrvDimension(6);
/// The header's `D3D12_SRV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURE2DMSARRAY: SrvDimension = SrvDimension(7);
/// The header's `D3D12_SRV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURE3D: SrvDimension = SrvDimension(8);
/// The header's `D3D12_SRV_DIMENSION_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURECUBE: SrvDimension = SrvDimension(9);
/// The header's `D3D12_SRV_DIMENSION_TEXTURECUBEARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_TEXTURECUBEARRAY: SrvDimension = SrvDimension(10);
/// The header's `D3D12_SRV_DIMENSION_RAYTRACING_ACCELERATION_STRUCTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_srv_dimension).
pub const D3D12_SRV_DIMENSION_RAYTRACING_ACCELERATION_STRUCTURE: SrvDimension = SrvDimension(11);

/// The header's `D3D12_STANDARD_MULTISAMPLE_QUALITY_LEVELS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StandardMultisampleQualityLevels(pub i32);

/// The header's `D3D12_STANDARD_MULTISAMPLE_PATTERN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_STANDARD_MULTISAMPLE_PATTERN: StandardMultisampleQualityLevels =
    StandardMultisampleQualityLevels(-1);
/// The header's `D3D12_CENTER_MULTISAMPLE_PATTERN`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_CENTER_MULTISAMPLE_PATTERN: StandardMultisampleQualityLevels =
    StandardMultisampleQualityLevels(-2);

/// The header's `D3D12_STATE_OBJECT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StateObjectFlags(pub i32);

/// The header's `D3D12_STATE_OBJECT_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_flags).
pub const D3D12_STATE_OBJECT_FLAG_NONE: StateObjectFlags = StateObjectFlags(0);
/// The header's `D3D12_STATE_OBJECT_FLAG_ALLOW_LOCAL_DEPENDENCIES_ON_EXTERNAL_DEFINITIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_flags).
pub const D3D12_STATE_OBJECT_FLAG_ALLOW_LOCAL_DEPENDENCIES_ON_EXTERNAL_DEFINITIONS:
    StateObjectFlags = StateObjectFlags(1);
/// The header's `D3D12_STATE_OBJECT_FLAG_ALLOW_EXTERNAL_DEPENDENCIES_ON_LOCAL_DEFINITIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_flags).
pub const D3D12_STATE_OBJECT_FLAG_ALLOW_EXTERNAL_DEPENDENCIES_ON_LOCAL_DEFINITIONS:
    StateObjectFlags = StateObjectFlags(2);
/// The header's `D3D12_STATE_OBJECT_FLAG_ALLOW_STATE_OBJECT_ADDITIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_flags).
pub const D3D12_STATE_OBJECT_FLAG_ALLOW_STATE_OBJECT_ADDITIONS: StateObjectFlags =
    StateObjectFlags(4);

/// The header's `D3D12_STATE_OBJECT_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StateObjectType(pub i32);

/// The header's `D3D12_STATE_OBJECT_TYPE_COLLECTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_type).
pub const D3D12_STATE_OBJECT_TYPE_COLLECTION: StateObjectType = StateObjectType(0);
/// The header's `D3D12_STATE_OBJECT_TYPE_RAYTRACING_PIPELINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_type).
pub const D3D12_STATE_OBJECT_TYPE_RAYTRACING_PIPELINE: StateObjectType = StateObjectType(3);
/// The header's `D3D12_STATE_OBJECT_TYPE_EXECUTABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_object_type).
pub const D3D12_STATE_OBJECT_TYPE_EXECUTABLE: StateObjectType = StateObjectType(4);

/// The header's `D3D12_STATE_SUBOBJECT_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StateSubobjectType(pub i32);

/// The header's `D3D12_STATE_SUBOBJECT_TYPE_STATE_OBJECT_CONFIG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_STATE_OBJECT_CONFIG: StateSubobjectType =
    StateSubobjectType(0);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_GLOBAL_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_GLOBAL_ROOT_SIGNATURE: StateSubobjectType =
    StateSubobjectType(1);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_LOCAL_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_LOCAL_ROOT_SIGNATURE: StateSubobjectType =
    StateSubobjectType(2);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_NODE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_NODE_MASK: StateSubobjectType = StateSubobjectType(3);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_DXIL_LIBRARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_DXIL_LIBRARY: StateSubobjectType = StateSubobjectType(5);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_EXISTING_COLLECTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_EXISTING_COLLECTION: StateSubobjectType =
    StateSubobjectType(6);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_SUBOBJECT_TO_EXPORTS_ASSOCIATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_SUBOBJECT_TO_EXPORTS_ASSOCIATION: StateSubobjectType =
    StateSubobjectType(7);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_DXIL_SUBOBJECT_TO_EXPORTS_ASSOCIATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_DXIL_SUBOBJECT_TO_EXPORTS_ASSOCIATION: StateSubobjectType =
    StateSubobjectType(8);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_RAYTRACING_SHADER_CONFIG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_RAYTRACING_SHADER_CONFIG: StateSubobjectType =
    StateSubobjectType(9);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_RAYTRACING_PIPELINE_CONFIG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_RAYTRACING_PIPELINE_CONFIG: StateSubobjectType =
    StateSubobjectType(10);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_HIT_GROUP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_HIT_GROUP: StateSubobjectType = StateSubobjectType(11);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_RAYTRACING_PIPELINE_CONFIG1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_RAYTRACING_PIPELINE_CONFIG1: StateSubobjectType =
    StateSubobjectType(12);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_WORK_GRAPH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_WORK_GRAPH: StateSubobjectType = StateSubobjectType(13);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_STREAM_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_STREAM_OUTPUT: StateSubobjectType = StateSubobjectType(14);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_BLEND: StateSubobjectType = StateSubobjectType(15);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_SAMPLE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_SAMPLE_MASK: StateSubobjectType = StateSubobjectType(16);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_RASTERIZER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_RASTERIZER: StateSubobjectType = StateSubobjectType(17);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL: StateSubobjectType = StateSubobjectType(18);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_INPUT_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_INPUT_LAYOUT: StateSubobjectType = StateSubobjectType(19);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_IB_STRIP_CUT_VALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_IB_STRIP_CUT_VALUE: StateSubobjectType =
    StateSubobjectType(20);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY: StateSubobjectType =
    StateSubobjectType(21);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS: StateSubobjectType =
    StateSubobjectType(22);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT: StateSubobjectType =
    StateSubobjectType(23);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_SAMPLE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_SAMPLE_DESC: StateSubobjectType = StateSubobjectType(24);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_FLAGS: StateSubobjectType = StateSubobjectType(26);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1: StateSubobjectType = StateSubobjectType(27);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING: StateSubobjectType = StateSubobjectType(28);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_GENERIC_PROGRAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_GENERIC_PROGRAM: StateSubobjectType = StateSubobjectType(29);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL2: StateSubobjectType = StateSubobjectType(30);
/// The header's `D3D12_STATE_SUBOBJECT_TYPE_MAX_VALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_state_subobject_type).
pub const D3D12_STATE_SUBOBJECT_TYPE_MAX_VALID: StateSubobjectType = StateSubobjectType(31);

/// The header's `D3D12_STATIC_BORDER_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_static_border_color).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StaticBorderColor(pub i32);

/// The header's `D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_static_border_color).
pub const D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK: StaticBorderColor = StaticBorderColor(0);
/// The header's `D3D12_STATIC_BORDER_COLOR_OPAQUE_BLACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_static_border_color).
pub const D3D12_STATIC_BORDER_COLOR_OPAQUE_BLACK: StaticBorderColor = StaticBorderColor(1);
/// The header's `D3D12_STATIC_BORDER_COLOR_OPAQUE_WHITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_static_border_color).
pub const D3D12_STATIC_BORDER_COLOR_OPAQUE_WHITE: StaticBorderColor = StaticBorderColor(2);
/// The header's `D3D12_STATIC_BORDER_COLOR_OPAQUE_BLACK_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_static_border_color).
pub const D3D12_STATIC_BORDER_COLOR_OPAQUE_BLACK_UINT: StaticBorderColor = StaticBorderColor(3);
/// The header's `D3D12_STATIC_BORDER_COLOR_OPAQUE_WHITE_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_static_border_color).
pub const D3D12_STATIC_BORDER_COLOR_OPAQUE_WHITE_UINT: StaticBorderColor = StaticBorderColor(4);

/// The header's `D3D12_STENCIL_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StencilOp(pub i32);

/// The header's `D3D12_STENCIL_OP_KEEP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
pub const D3D12_STENCIL_OP_KEEP: StencilOp = StencilOp(1);
/// The header's `D3D12_STENCIL_OP_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
pub const D3D12_STENCIL_OP_ZERO: StencilOp = StencilOp(2);
/// The header's `D3D12_STENCIL_OP_REPLACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
pub const D3D12_STENCIL_OP_REPLACE: StencilOp = StencilOp(3);
/// The header's `D3D12_STENCIL_OP_INCR_SAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
pub const D3D12_STENCIL_OP_INCR_SAT: StencilOp = StencilOp(4);
/// The header's `D3D12_STENCIL_OP_DECR_SAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
pub const D3D12_STENCIL_OP_DECR_SAT: StencilOp = StencilOp(5);
/// The header's `D3D12_STENCIL_OP_INVERT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
pub const D3D12_STENCIL_OP_INVERT: StencilOp = StencilOp(6);
/// The header's `D3D12_STENCIL_OP_INCR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
pub const D3D12_STENCIL_OP_INCR: StencilOp = StencilOp(7);
/// The header's `D3D12_STENCIL_OP_DECR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_stencil_op).
pub const D3D12_STENCIL_OP_DECR: StencilOp = StencilOp(8);

/// The header's `D3D12_TEXTURE_ADDRESS_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_address_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureAddressMode(pub i32);

/// The header's `D3D12_TEXTURE_ADDRESS_MODE_WRAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_address_mode).
pub const D3D12_TEXTURE_ADDRESS_MODE_WRAP: TextureAddressMode = TextureAddressMode(1);
/// The header's `D3D12_TEXTURE_ADDRESS_MODE_MIRROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_address_mode).
pub const D3D12_TEXTURE_ADDRESS_MODE_MIRROR: TextureAddressMode = TextureAddressMode(2);
/// The header's `D3D12_TEXTURE_ADDRESS_MODE_CLAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_address_mode).
pub const D3D12_TEXTURE_ADDRESS_MODE_CLAMP: TextureAddressMode = TextureAddressMode(3);
/// The header's `D3D12_TEXTURE_ADDRESS_MODE_BORDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_address_mode).
pub const D3D12_TEXTURE_ADDRESS_MODE_BORDER: TextureAddressMode = TextureAddressMode(4);
/// The header's `D3D12_TEXTURE_ADDRESS_MODE_MIRROR_ONCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_address_mode).
pub const D3D12_TEXTURE_ADDRESS_MODE_MIRROR_ONCE: TextureAddressMode = TextureAddressMode(5);

/// The header's `D3D12_TEXTURE_BARRIER_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureBarrierFlags(pub i32);

/// The header's `D3D12_TEXTURE_BARRIER_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_TEXTURE_BARRIER_FLAG_NONE: TextureBarrierFlags = TextureBarrierFlags(0);
/// The header's `D3D12_TEXTURE_BARRIER_FLAG_DISCARD`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_TEXTURE_BARRIER_FLAG_DISCARD: TextureBarrierFlags = TextureBarrierFlags(1);

/// The header's `D3D12_TEXTURE_COPY_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_copy_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureCopyType(pub i32);

/// The header's `D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_copy_type).
pub const D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX: TextureCopyType = TextureCopyType(0);
/// The header's `D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_copy_type).
pub const D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT: TextureCopyType = TextureCopyType(1);

/// The header's `D3D12_TEXTURE_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureLayout(pub i32);

/// The header's `D3D12_TEXTURE_LAYOUT_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_layout).
pub const D3D12_TEXTURE_LAYOUT_UNKNOWN: TextureLayout = TextureLayout(0);
/// The header's `D3D12_TEXTURE_LAYOUT_ROW_MAJOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_layout).
pub const D3D12_TEXTURE_LAYOUT_ROW_MAJOR: TextureLayout = TextureLayout(1);
/// The header's `D3D12_TEXTURE_LAYOUT_64KB_UNDEFINED_SWIZZLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_layout).
pub const D3D12_TEXTURE_LAYOUT_64KB_UNDEFINED_SWIZZLE: TextureLayout = TextureLayout(2);
/// The header's `D3D12_TEXTURE_LAYOUT_64KB_STANDARD_SWIZZLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_texture_layout).
pub const D3D12_TEXTURE_LAYOUT_64KB_STANDARD_SWIZZLE: TextureLayout = TextureLayout(3);

/// The header's `D3D12_TILED_RESOURCES_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tiled_resources_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TiledResourcesTier(pub i32);

/// The header's `D3D12_TILED_RESOURCES_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tiled_resources_tier).
pub const D3D12_TILED_RESOURCES_TIER_NOT_SUPPORTED: TiledResourcesTier = TiledResourcesTier(0);
/// The header's `D3D12_TILED_RESOURCES_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tiled_resources_tier).
pub const D3D12_TILED_RESOURCES_TIER_1: TiledResourcesTier = TiledResourcesTier(1);
/// The header's `D3D12_TILED_RESOURCES_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tiled_resources_tier).
pub const D3D12_TILED_RESOURCES_TIER_2: TiledResourcesTier = TiledResourcesTier(2);
/// The header's `D3D12_TILED_RESOURCES_TIER_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tiled_resources_tier).
pub const D3D12_TILED_RESOURCES_TIER_3: TiledResourcesTier = TiledResourcesTier(3);
/// The header's `D3D12_TILED_RESOURCES_TIER_4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tiled_resources_tier).
pub const D3D12_TILED_RESOURCES_TIER_4: TiledResourcesTier = TiledResourcesTier(4);

/// The header's `D3D12_TILE_COPY_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_copy_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TileCopyFlags(pub i32);

/// The header's `D3D12_TILE_COPY_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_copy_flags).
pub const D3D12_TILE_COPY_FLAG_NONE: TileCopyFlags = TileCopyFlags(0);
/// The header's `D3D12_TILE_COPY_FLAG_NO_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_copy_flags).
pub const D3D12_TILE_COPY_FLAG_NO_HAZARD: TileCopyFlags = TileCopyFlags(1);
/// The header's `D3D12_TILE_COPY_FLAG_LINEAR_BUFFER_TO_SWIZZLED_TILED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_copy_flags).
pub const D3D12_TILE_COPY_FLAG_LINEAR_BUFFER_TO_SWIZZLED_TILED_RESOURCE: TileCopyFlags =
    TileCopyFlags(2);
/// The header's `D3D12_TILE_COPY_FLAG_SWIZZLED_TILED_RESOURCE_TO_LINEAR_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_copy_flags).
pub const D3D12_TILE_COPY_FLAG_SWIZZLED_TILED_RESOURCE_TO_LINEAR_BUFFER: TileCopyFlags =
    TileCopyFlags(4);

/// The header's `D3D12_TILE_MAPPING_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_mapping_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TileMappingFlags(pub i32);

/// The header's `D3D12_TILE_MAPPING_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_mapping_flags).
pub const D3D12_TILE_MAPPING_FLAG_NONE: TileMappingFlags = TileMappingFlags(0);
/// The header's `D3D12_TILE_MAPPING_FLAG_NO_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_mapping_flags).
pub const D3D12_TILE_MAPPING_FLAG_NO_HAZARD: TileMappingFlags = TileMappingFlags(1);

/// The header's `D3D12_TILE_RANGE_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_range_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TileRangeFlags(pub i32);

/// The header's `D3D12_TILE_RANGE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_range_flags).
pub const D3D12_TILE_RANGE_FLAG_NONE: TileRangeFlags = TileRangeFlags(0);
/// The header's `D3D12_TILE_RANGE_FLAG_NULL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_range_flags).
pub const D3D12_TILE_RANGE_FLAG_NULL: TileRangeFlags = TileRangeFlags(1);
/// The header's `D3D12_TILE_RANGE_FLAG_SKIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_range_flags).
pub const D3D12_TILE_RANGE_FLAG_SKIP: TileRangeFlags = TileRangeFlags(2);
/// The header's `D3D12_TILE_RANGE_FLAG_REUSE_SINGLE_TILE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tile_range_flags).
pub const D3D12_TILE_RANGE_FLAG_REUSE_SINGLE_TILE: TileRangeFlags = TileRangeFlags(4);

/// The header's `D3D12_TRI_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tri_state).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TriState(pub i32);

/// The header's `D3D12_TRI_STATE_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tri_state).
pub const D3D12_TRI_STATE_UNKNOWN: TriState = TriState(-1);
/// The header's `D3D12_TRI_STATE_FALSE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tri_state).
pub const D3D12_TRI_STATE_FALSE: TriState = TriState(0);
/// The header's `D3D12_TRI_STATE_TRUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_tri_state).
pub const D3D12_TRI_STATE_TRUE: TriState = TriState(1);

/// The header's `D3D12_UAV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UavDimension(pub i32);

/// The header's `D3D12_UAV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_UNKNOWN: UavDimension = UavDimension(0);
/// The header's `D3D12_UAV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_BUFFER: UavDimension = UavDimension(1);
/// The header's `D3D12_UAV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_TEXTURE1D: UavDimension = UavDimension(2);
/// The header's `D3D12_UAV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_TEXTURE1DARRAY: UavDimension = UavDimension(3);
/// The header's `D3D12_UAV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_TEXTURE2D: UavDimension = UavDimension(4);
/// The header's `D3D12_UAV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_TEXTURE2DARRAY: UavDimension = UavDimension(5);
/// The header's `D3D12_UAV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_TEXTURE2DMS: UavDimension = UavDimension(6);
/// The header's `D3D12_UAV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_TEXTURE2DMSARRAY: UavDimension = UavDimension(7);
/// The header's `D3D12_UAV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_uav_dimension).
pub const D3D12_UAV_DIMENSION_TEXTURE3D: UavDimension = UavDimension(8);

/// The header's `D3D12_VARIABLE_SHADING_RATE_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_variable_shading_rate_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VariableShadingRateTier(pub i32);

/// The header's `D3D12_VARIABLE_SHADING_RATE_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_variable_shading_rate_tier).
pub const D3D12_VARIABLE_SHADING_RATE_TIER_NOT_SUPPORTED: VariableShadingRateTier =
    VariableShadingRateTier(0);
/// The header's `D3D12_VARIABLE_SHADING_RATE_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_variable_shading_rate_tier).
pub const D3D12_VARIABLE_SHADING_RATE_TIER_1: VariableShadingRateTier = VariableShadingRateTier(1);
/// The header's `D3D12_VARIABLE_SHADING_RATE_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_variable_shading_rate_tier).
pub const D3D12_VARIABLE_SHADING_RATE_TIER_2: VariableShadingRateTier = VariableShadingRateTier(2);

/// The header's `D3D12_VIEW_INSTANCING_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_view_instancing_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ViewInstancingFlags(pub i32);

/// The header's `D3D12_VIEW_INSTANCING_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_view_instancing_flags).
pub const D3D12_VIEW_INSTANCING_FLAG_NONE: ViewInstancingFlags = ViewInstancingFlags(0);
/// The header's `D3D12_VIEW_INSTANCING_FLAG_ENABLE_VIEW_INSTANCE_MASKING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_view_instancing_flags).
pub const D3D12_VIEW_INSTANCING_FLAG_ENABLE_VIEW_INSTANCE_MASKING: ViewInstancingFlags =
    ViewInstancingFlags(1);

/// The header's `D3D12_VIEW_INSTANCING_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_view_instancing_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ViewInstancingTier(pub i32);

/// The header's `D3D12_VIEW_INSTANCING_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_view_instancing_tier).
pub const D3D12_VIEW_INSTANCING_TIER_NOT_SUPPORTED: ViewInstancingTier = ViewInstancingTier(0);
/// The header's `D3D12_VIEW_INSTANCING_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_view_instancing_tier).
pub const D3D12_VIEW_INSTANCING_TIER_1: ViewInstancingTier = ViewInstancingTier(1);
/// The header's `D3D12_VIEW_INSTANCING_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_view_instancing_tier).
pub const D3D12_VIEW_INSTANCING_TIER_2: ViewInstancingTier = ViewInstancingTier(2);
/// The header's `D3D12_VIEW_INSTANCING_TIER_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_view_instancing_tier).
pub const D3D12_VIEW_INSTANCING_TIER_3: ViewInstancingTier = ViewInstancingTier(3);

/// The header's `D3D12_WAVE_MMA_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_wave_mma_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WaveMmaTier(pub i32);

/// The header's `D3D12_WAVE_MMA_TIER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_wave_mma_tier).
pub const D3D12_WAVE_MMA_TIER_NOT_SUPPORTED: WaveMmaTier = WaveMmaTier(0);
/// The header's `D3D12_WAVE_MMA_TIER_1_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_wave_mma_tier).
pub const D3D12_WAVE_MMA_TIER_1_0: WaveMmaTier = WaveMmaTier(10);

/// The header's `D3D12_WORK_GRAPHS_TIER`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorkGraphsTier(pub i32);

/// The header's `D3D12_WORK_GRAPHS_TIER_NOT_SUPPORTED`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_WORK_GRAPHS_TIER_NOT_SUPPORTED: WorkGraphsTier = WorkGraphsTier(0);
/// The header's `D3D12_WORK_GRAPHS_TIER_1_0`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_WORK_GRAPHS_TIER_1_0: WorkGraphsTier = WorkGraphsTier(10);

/// The header's `D3D12_WORK_GRAPH_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorkGraphFlags(pub i32);

/// The header's `D3D12_WORK_GRAPH_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_WORK_GRAPH_FLAG_NONE: WorkGraphFlags = WorkGraphFlags(0);
/// The header's `D3D12_WORK_GRAPH_FLAG_INCLUDE_ALL_AVAILABLE_NODES`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D12_WORK_GRAPH_FLAG_INCLUDE_ALL_AVAILABLE_NODES: WorkGraphFlags = WorkGraphFlags(1);

/// The header's `D3D12_WRITEBUFFERIMMEDIATE_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_writebufferimmediate_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WritebufferimmediateMode(pub i32);

/// The header's `D3D12_WRITEBUFFERIMMEDIATE_MODE_DEFAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_writebufferimmediate_mode).
pub const D3D12_WRITEBUFFERIMMEDIATE_MODE_DEFAULT: WritebufferimmediateMode =
    WritebufferimmediateMode(0);
/// The header's `D3D12_WRITEBUFFERIMMEDIATE_MODE_MARKER_IN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_writebufferimmediate_mode).
pub const D3D12_WRITEBUFFERIMMEDIATE_MODE_MARKER_IN: WritebufferimmediateMode =
    WritebufferimmediateMode(1);
/// The header's `D3D12_WRITEBUFFERIMMEDIATE_MODE_MARKER_OUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d12_writebufferimmediate_mode).
pub const D3D12_WRITEBUFFERIMMEDIATE_MODE_MARKER_OUT: WritebufferimmediateMode =
    WritebufferimmediateMode(2);

/// The header's `D3D_ROOT_SIGNATURE_VERSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_root_signature_version).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RootSignatureVersion(pub i32);

/// The header's `D3D_ROOT_SIGNATURE_VERSION_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_root_signature_version).
pub const D3D_ROOT_SIGNATURE_VERSION_1: RootSignatureVersion = RootSignatureVersion(1);
/// The header's `D3D_ROOT_SIGNATURE_VERSION_1_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_root_signature_version).
pub const D3D_ROOT_SIGNATURE_VERSION_1_0: RootSignatureVersion = RootSignatureVersion(1);
/// The header's `D3D_ROOT_SIGNATURE_VERSION_1_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_root_signature_version).
pub const D3D_ROOT_SIGNATURE_VERSION_1_1: RootSignatureVersion = RootSignatureVersion(2);
/// The header's `D3D_ROOT_SIGNATURE_VERSION_1_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_root_signature_version).
pub const D3D_ROOT_SIGNATURE_VERSION_1_2: RootSignatureVersion = RootSignatureVersion(3);

/// The header's `D3D_SHADER_MODEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderModel(pub i32);

/// The header's `D3D_SHADER_MODEL_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_NONE: ShaderModel = ShaderModel(0);
/// The header's `D3D_SHADER_MODEL_5_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_5_1: ShaderModel = ShaderModel(81);
/// The header's `D3D_SHADER_MODEL_6_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_0: ShaderModel = ShaderModel(96);
/// The header's `D3D_SHADER_MODEL_6_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_1: ShaderModel = ShaderModel(97);
/// The header's `D3D_SHADER_MODEL_6_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_2: ShaderModel = ShaderModel(98);
/// The header's `D3D_SHADER_MODEL_6_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_3: ShaderModel = ShaderModel(99);
/// The header's `D3D_SHADER_MODEL_6_4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_4: ShaderModel = ShaderModel(100);
/// The header's `D3D_SHADER_MODEL_6_5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_5: ShaderModel = ShaderModel(101);
/// The header's `D3D_SHADER_MODEL_6_6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_6: ShaderModel = ShaderModel(102);
/// The header's `D3D_SHADER_MODEL_6_7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_7: ShaderModel = ShaderModel(103);
/// The header's `D3D_SHADER_MODEL_6_8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_8: ShaderModel = ShaderModel(104);
/// The header's `D3D_SHADER_MODEL_6_9`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_SHADER_MODEL_6_9: ShaderModel = ShaderModel(105);
/// The header's `D3D_HIGHEST_SHADER_MODEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model).
pub const D3D_HIGHEST_SHADER_MODEL: ShaderModel = ShaderModel(105);
