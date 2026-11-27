//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

/// The header's `D3D11_1_CREATE_DEVICE_CONTEXT_STATE_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_1_create_device_context_state_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct D3D111CreateDeviceContextStateFlag(pub i32);

/// The header's `D3D11_1_CREATE_DEVICE_CONTEXT_STATE_SINGLETHREADED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_1_create_device_context_state_flag).
pub const D3D11_1_CREATE_DEVICE_CONTEXT_STATE_SINGLETHREADED: D3D111CreateDeviceContextStateFlag =
    D3D111CreateDeviceContextStateFlag(1);

/// The header's `D3D11_ASYNC_GETDATA_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_async_getdata_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AsyncGetdataFlag(pub i32);

/// The header's `D3D11_ASYNC_GETDATA_DONOTFLUSH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_async_getdata_flag).
pub const D3D11_ASYNC_GETDATA_DONOTFLUSH: AsyncGetdataFlag = AsyncGetdataFlag(1);

/// The header's `D3D11_AUTHENTICATED_CHANNEL_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_authenticated_channel_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AuthenticatedChannelType(pub i32);

/// The header's `D3D11_AUTHENTICATED_CHANNEL_D3D11`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_authenticated_channel_type).
pub const D3D11_AUTHENTICATED_CHANNEL_D3D11: AuthenticatedChannelType = AuthenticatedChannelType(1);
/// The header's `D3D11_AUTHENTICATED_CHANNEL_DRIVER_SOFTWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_authenticated_channel_type).
pub const D3D11_AUTHENTICATED_CHANNEL_DRIVER_SOFTWARE: AuthenticatedChannelType =
    AuthenticatedChannelType(2);
/// The header's `D3D11_AUTHENTICATED_CHANNEL_DRIVER_HARDWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_authenticated_channel_type).
pub const D3D11_AUTHENTICATED_CHANNEL_DRIVER_HARDWARE: AuthenticatedChannelType =
    AuthenticatedChannelType(3);

/// The header's `D3D11_AUTHENTICATED_PROCESS_IDENTIFIER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_authenticated_process_identifier_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AuthenticatedProcessIdentifierType(pub i32);

/// The header's `D3D11_PROCESSIDTYPE_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_authenticated_process_identifier_type).
pub const D3D11_PROCESSIDTYPE_UNKNOWN: AuthenticatedProcessIdentifierType =
    AuthenticatedProcessIdentifierType(0);
/// The header's `D3D11_PROCESSIDTYPE_DWM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_authenticated_process_identifier_type).
pub const D3D11_PROCESSIDTYPE_DWM: AuthenticatedProcessIdentifierType =
    AuthenticatedProcessIdentifierType(1);
/// The header's `D3D11_PROCESSIDTYPE_HANDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_authenticated_process_identifier_type).
pub const D3D11_PROCESSIDTYPE_HANDLE: AuthenticatedProcessIdentifierType =
    AuthenticatedProcessIdentifierType(2);

/// The header's `D3D11_BIND_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BindFlag(pub i32);

/// The header's `D3D11_BIND_VERTEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_VERTEX_BUFFER: BindFlag = BindFlag(1);
/// The header's `D3D11_BIND_INDEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_INDEX_BUFFER: BindFlag = BindFlag(2);
/// The header's `D3D11_BIND_CONSTANT_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_CONSTANT_BUFFER: BindFlag = BindFlag(4);
/// The header's `D3D11_BIND_SHADER_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_SHADER_RESOURCE: BindFlag = BindFlag(8);
/// The header's `D3D11_BIND_STREAM_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_STREAM_OUTPUT: BindFlag = BindFlag(16);
/// The header's `D3D11_BIND_RENDER_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_RENDER_TARGET: BindFlag = BindFlag(32);
/// The header's `D3D11_BIND_DEPTH_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_DEPTH_STENCIL: BindFlag = BindFlag(64);
/// The header's `D3D11_BIND_UNORDERED_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_UNORDERED_ACCESS: BindFlag = BindFlag(128);
/// The header's `D3D11_BIND_DECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_DECODER: BindFlag = BindFlag(0x200);
/// The header's `D3D11_BIND_VIDEO_ENCODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag).
pub const D3D11_BIND_VIDEO_ENCODER: BindFlag = BindFlag(0x400);

/// The header's `D3D11_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Blend(pub i32);

/// The header's `D3D11_BLEND_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_ZERO: Blend = Blend(1);
/// The header's `D3D11_BLEND_ONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_ONE: Blend = Blend(2);
/// The header's `D3D11_BLEND_SRC_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_SRC_COLOR: Blend = Blend(3);
/// The header's `D3D11_BLEND_INV_SRC_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_INV_SRC_COLOR: Blend = Blend(4);
/// The header's `D3D11_BLEND_SRC_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_SRC_ALPHA: Blend = Blend(5);
/// The header's `D3D11_BLEND_INV_SRC_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_INV_SRC_ALPHA: Blend = Blend(6);
/// The header's `D3D11_BLEND_DEST_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_DEST_ALPHA: Blend = Blend(7);
/// The header's `D3D11_BLEND_INV_DEST_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_INV_DEST_ALPHA: Blend = Blend(8);
/// The header's `D3D11_BLEND_DEST_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_DEST_COLOR: Blend = Blend(9);
/// The header's `D3D11_BLEND_INV_DEST_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_INV_DEST_COLOR: Blend = Blend(10);
/// The header's `D3D11_BLEND_SRC_ALPHA_SAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_SRC_ALPHA_SAT: Blend = Blend(11);
/// The header's `D3D11_BLEND_BLEND_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_BLEND_FACTOR: Blend = Blend(14);
/// The header's `D3D11_BLEND_INV_BLEND_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_INV_BLEND_FACTOR: Blend = Blend(15);
/// The header's `D3D11_BLEND_SRC1_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_SRC1_COLOR: Blend = Blend(16);
/// The header's `D3D11_BLEND_INV_SRC1_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_INV_SRC1_COLOR: Blend = Blend(17);
/// The header's `D3D11_BLEND_SRC1_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_SRC1_ALPHA: Blend = Blend(18);
/// The header's `D3D11_BLEND_INV_SRC1_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend).
pub const D3D11_BLEND_INV_SRC1_ALPHA: Blend = Blend(19);

/// The header's `D3D11_BLEND_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend_op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BlendOp(pub i32);

/// The header's `D3D11_BLEND_OP_ADD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend_op).
pub const D3D11_BLEND_OP_ADD: BlendOp = BlendOp(1);
/// The header's `D3D11_BLEND_OP_SUBTRACT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend_op).
pub const D3D11_BLEND_OP_SUBTRACT: BlendOp = BlendOp(2);
/// The header's `D3D11_BLEND_OP_REV_SUBTRACT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend_op).
pub const D3D11_BLEND_OP_REV_SUBTRACT: BlendOp = BlendOp(3);
/// The header's `D3D11_BLEND_OP_MIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend_op).
pub const D3D11_BLEND_OP_MIN: BlendOp = BlendOp(4);
/// The header's `D3D11_BLEND_OP_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_blend_op).
pub const D3D11_BLEND_OP_MAX: BlendOp = BlendOp(5);

/// The header's `D3D11_BUFFEREX_SRV_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bufferex_srv_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BufferexSrvFlag(pub i32);

/// The header's `D3D11_BUFFEREX_SRV_FLAG_RAW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bufferex_srv_flag).
pub const D3D11_BUFFEREX_SRV_FLAG_RAW: BufferexSrvFlag = BufferexSrvFlag(1);

/// The header's `D3D11_BUFFER_UAV_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_buffer_uav_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BufferUavFlag(pub i32);

/// The header's `D3D11_BUFFER_UAV_FLAG_RAW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_buffer_uav_flag).
pub const D3D11_BUFFER_UAV_FLAG_RAW: BufferUavFlag = BufferUavFlag(1);
/// The header's `D3D11_BUFFER_UAV_FLAG_APPEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_buffer_uav_flag).
pub const D3D11_BUFFER_UAV_FLAG_APPEND: BufferUavFlag = BufferUavFlag(2);
/// The header's `D3D11_BUFFER_UAV_FLAG_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_buffer_uav_flag).
pub const D3D11_BUFFER_UAV_FLAG_COUNTER: BufferUavFlag = BufferUavFlag(4);

/// The header's `D3D11_BUS_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BusType(pub i32);

/// The header's `D3D11_BUS_TYPE_OTHER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_TYPE_OTHER: BusType = BusType(0);
/// The header's `D3D11_BUS_TYPE_PCI`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_TYPE_PCI: BusType = BusType(1);
/// The header's `D3D11_BUS_TYPE_PCIX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_TYPE_PCIX: BusType = BusType(2);
/// The header's `D3D11_BUS_TYPE_PCIEXPRESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_TYPE_PCIEXPRESS: BusType = BusType(3);
/// The header's `D3D11_BUS_TYPE_AGP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_TYPE_AGP: BusType = BusType(4);
/// The header's `D3D11_BUS_IMPL_MODIFIER_INSIDE_OF_CHIPSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_IMPL_MODIFIER_INSIDE_OF_CHIPSET: BusType = BusType(0x10000);
/// The header's `D3D11_BUS_IMPL_MODIFIER_TRACKS_ON_MOTHER_BOARD_TO_CHIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_IMPL_MODIFIER_TRACKS_ON_MOTHER_BOARD_TO_CHIP: BusType = BusType(0x20000);
/// The header's `D3D11_BUS_IMPL_MODIFIER_TRACKS_ON_MOTHER_BOARD_TO_SOCKET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_IMPL_MODIFIER_TRACKS_ON_MOTHER_BOARD_TO_SOCKET: BusType = BusType(0x30000);
/// The header's `D3D11_BUS_IMPL_MODIFIER_DAUGHTER_BOARD_CONNECTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_IMPL_MODIFIER_DAUGHTER_BOARD_CONNECTOR: BusType = BusType(0x40000);
/// The header's `D3D11_BUS_IMPL_MODIFIER_DAUGHTER_BOARD_CONNECTOR_INSIDE_OF_NUAE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_IMPL_MODIFIER_DAUGHTER_BOARD_CONNECTOR_INSIDE_OF_NUAE: BusType =
    BusType(0x50000);
/// The header's `D3D11_BUS_IMPL_MODIFIER_NON_STANDARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bus_type).
pub const D3D11_BUS_IMPL_MODIFIER_NON_STANDARD: BusType = BusType(-2147483648);

/// The header's `D3D11_CHECK_MULTISAMPLE_QUALITY_LEVELS_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_check_multisample_quality_levels_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CheckMultisampleQualityLevelsFlag(pub i32);

/// The header's `D3D11_CHECK_MULTISAMPLE_QUALITY_LEVELS_TILED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_check_multisample_quality_levels_flag).
pub const D3D11_CHECK_MULTISAMPLE_QUALITY_LEVELS_TILED_RESOURCE: CheckMultisampleQualityLevelsFlag =
    CheckMultisampleQualityLevelsFlag(1);

/// The header's `D3D11_CLEAR_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_clear_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClearFlag(pub u32);

/// The header's `D3D11_CLEAR_DEPTH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_clear_flag).
pub const D3D11_CLEAR_DEPTH: ClearFlag = ClearFlag(1);
/// The header's `D3D11_CLEAR_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_clear_flag).
pub const D3D11_CLEAR_STENCIL: ClearFlag = ClearFlag(2);

/// The header's `D3D11_COLOR_WRITE_ENABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_color_write_enable).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ColorWriteEnable(pub i32);

/// The header's `D3D11_COLOR_WRITE_ENABLE_RED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_color_write_enable).
pub const D3D11_COLOR_WRITE_ENABLE_RED: ColorWriteEnable = ColorWriteEnable(1);
/// The header's `D3D11_COLOR_WRITE_ENABLE_GREEN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_color_write_enable).
pub const D3D11_COLOR_WRITE_ENABLE_GREEN: ColorWriteEnable = ColorWriteEnable(2);
/// The header's `D3D11_COLOR_WRITE_ENABLE_BLUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_color_write_enable).
pub const D3D11_COLOR_WRITE_ENABLE_BLUE: ColorWriteEnable = ColorWriteEnable(4);
/// The header's `D3D11_COLOR_WRITE_ENABLE_ALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_color_write_enable).
pub const D3D11_COLOR_WRITE_ENABLE_ALPHA: ColorWriteEnable = ColorWriteEnable(8);
/// The header's `D3D11_COLOR_WRITE_ENABLE_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_color_write_enable).
pub const D3D11_COLOR_WRITE_ENABLE_ALL: ColorWriteEnable = ColorWriteEnable(15);

/// The header's `D3D11_COMPARISON_FUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ComparisonFunc(pub i32);

/// The header's `D3D11_COMPARISON_NEVER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
pub const D3D11_COMPARISON_NEVER: ComparisonFunc = ComparisonFunc(1);
/// The header's `D3D11_COMPARISON_LESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
pub const D3D11_COMPARISON_LESS: ComparisonFunc = ComparisonFunc(2);
/// The header's `D3D11_COMPARISON_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
pub const D3D11_COMPARISON_EQUAL: ComparisonFunc = ComparisonFunc(3);
/// The header's `D3D11_COMPARISON_LESS_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
pub const D3D11_COMPARISON_LESS_EQUAL: ComparisonFunc = ComparisonFunc(4);
/// The header's `D3D11_COMPARISON_GREATER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
pub const D3D11_COMPARISON_GREATER: ComparisonFunc = ComparisonFunc(5);
/// The header's `D3D11_COMPARISON_NOT_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
pub const D3D11_COMPARISON_NOT_EQUAL: ComparisonFunc = ComparisonFunc(6);
/// The header's `D3D11_COMPARISON_GREATER_EQUAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
pub const D3D11_COMPARISON_GREATER_EQUAL: ComparisonFunc = ComparisonFunc(7);
/// The header's `D3D11_COMPARISON_ALWAYS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_comparison_func).
pub const D3D11_COMPARISON_ALWAYS: ComparisonFunc = ComparisonFunc(8);

/// The header's `D3D11_CONSERVATIVE_RASTERIZATION_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_conservative_rasterization_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConservativeRasterizationMode(pub i32);

/// The header's `D3D11_CONSERVATIVE_RASTERIZATION_MODE_OFF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_conservative_rasterization_mode).
pub const D3D11_CONSERVATIVE_RASTERIZATION_MODE_OFF: ConservativeRasterizationMode =
    ConservativeRasterizationMode(0);
/// The header's `D3D11_CONSERVATIVE_RASTERIZATION_MODE_ON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_conservative_rasterization_mode).
pub const D3D11_CONSERVATIVE_RASTERIZATION_MODE_ON: ConservativeRasterizationMode =
    ConservativeRasterizationMode(1);

/// The header's `D3D11_CONSERVATIVE_RASTERIZATION_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_conservative_rasterization_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConservativeRasterizationTier(pub i32);

/// The header's `D3D11_CONSERVATIVE_RASTERIZATION_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_conservative_rasterization_tier).
pub const D3D11_CONSERVATIVE_RASTERIZATION_NOT_SUPPORTED: ConservativeRasterizationTier =
    ConservativeRasterizationTier(0);
/// The header's `D3D11_CONSERVATIVE_RASTERIZATION_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_conservative_rasterization_tier).
pub const D3D11_CONSERVATIVE_RASTERIZATION_TIER_1: ConservativeRasterizationTier =
    ConservativeRasterizationTier(1);
/// The header's `D3D11_CONSERVATIVE_RASTERIZATION_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_conservative_rasterization_tier).
pub const D3D11_CONSERVATIVE_RASTERIZATION_TIER_2: ConservativeRasterizationTier =
    ConservativeRasterizationTier(2);
/// The header's `D3D11_CONSERVATIVE_RASTERIZATION_TIER_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_conservative_rasterization_tier).
pub const D3D11_CONSERVATIVE_RASTERIZATION_TIER_3: ConservativeRasterizationTier =
    ConservativeRasterizationTier(3);

/// The header's `D3D11_CONTENT_PROTECTION_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ContentProtectionCaps(pub i32);

/// The header's `D3D11_CONTENT_PROTECTION_CAPS_SOFTWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_SOFTWARE: ContentProtectionCaps = ContentProtectionCaps(1);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_HARDWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_HARDWARE: ContentProtectionCaps = ContentProtectionCaps(2);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_PROTECTION_ALWAYS_ON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_PROTECTION_ALWAYS_ON: ContentProtectionCaps =
    ContentProtectionCaps(4);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_PARTIAL_DECRYPTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_PARTIAL_DECRYPTION: ContentProtectionCaps =
    ContentProtectionCaps(8);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_CONTENT_KEY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_CONTENT_KEY: ContentProtectionCaps =
    ContentProtectionCaps(16);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_FRESHEN_SESSION_KEY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_FRESHEN_SESSION_KEY: ContentProtectionCaps =
    ContentProtectionCaps(32);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_ENCRYPTED_READ_BACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_ENCRYPTED_READ_BACK: ContentProtectionCaps =
    ContentProtectionCaps(64);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_ENCRYPTED_READ_BACK_KEY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_ENCRYPTED_READ_BACK_KEY: ContentProtectionCaps =
    ContentProtectionCaps(128);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_SEQUENTIAL_CTR_IV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_SEQUENTIAL_CTR_IV: ContentProtectionCaps =
    ContentProtectionCaps(0x100);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_ENCRYPT_SLICEDATA_ONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_ENCRYPT_SLICEDATA_ONLY: ContentProtectionCaps =
    ContentProtectionCaps(0x200);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_DECRYPTION_BLT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_DECRYPTION_BLT: ContentProtectionCaps =
    ContentProtectionCaps(0x400);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_PROTECT_UNCOMPRESSED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_PROTECT_UNCOMPRESSED: ContentProtectionCaps =
    ContentProtectionCaps(0x800);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_PROTECTED_MEMORY_PAGEABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_PROTECTED_MEMORY_PAGEABLE: ContentProtectionCaps =
    ContentProtectionCaps(0x1000);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_TEARDOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_TEARDOWN: ContentProtectionCaps =
    ContentProtectionCaps(0x2000);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_DRM_COMMUNICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_DRM_COMMUNICATION: ContentProtectionCaps =
    ContentProtectionCaps(0x4000);
/// The header's `D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_DRM_COMMUNICATION_MULTI_THREADED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_content_protection_caps).
pub const D3D11_CONTENT_PROTECTION_CAPS_HARDWARE_DRM_COMMUNICATION_MULTI_THREADED:
    ContentProtectionCaps = ContentProtectionCaps(0x8000);

/// The header's `D3D11_CONTEXT_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_context_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ContextType(pub i32);

/// The header's `D3D11_CONTEXT_TYPE_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_context_type).
pub const D3D11_CONTEXT_TYPE_ALL: ContextType = ContextType(0);
/// The header's `D3D11_CONTEXT_TYPE_3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_context_type).
pub const D3D11_CONTEXT_TYPE_3D: ContextType = ContextType(1);
/// The header's `D3D11_CONTEXT_TYPE_COMPUTE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_context_type).
pub const D3D11_CONTEXT_TYPE_COMPUTE: ContextType = ContextType(2);
/// The header's `D3D11_CONTEXT_TYPE_COPY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_context_type).
pub const D3D11_CONTEXT_TYPE_COPY: ContextType = ContextType(3);
/// The header's `D3D11_CONTEXT_TYPE_VIDEO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_context_type).
pub const D3D11_CONTEXT_TYPE_VIDEO: ContextType = ContextType(4);

/// The header's `D3D11_COPY_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_copy_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CopyFlags(pub i32);

/// The header's `D3D11_COPY_NO_OVERWRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_copy_flags).
pub const D3D11_COPY_NO_OVERWRITE: CopyFlags = CopyFlags(1);
/// The header's `D3D11_COPY_DISCARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_copy_flags).
pub const D3D11_COPY_DISCARD: CopyFlags = CopyFlags(2);

/// The header's `D3D11_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_counter).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CounterKind(pub i32);

/// The header's `D3D11_COUNTER_DEVICE_DEPENDENT_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_counter).
pub const D3D11_COUNTER_DEVICE_DEPENDENT_0: CounterKind = CounterKind(0x40000000);

/// The header's `D3D11_COUNTER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_counter_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CounterType(pub i32);

/// The header's `D3D11_COUNTER_TYPE_FLOAT32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_counter_type).
pub const D3D11_COUNTER_TYPE_FLOAT32: CounterType = CounterType(0);
/// The header's `D3D11_COUNTER_TYPE_UINT16`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_counter_type).
pub const D3D11_COUNTER_TYPE_UINT16: CounterType = CounterType(1);
/// The header's `D3D11_COUNTER_TYPE_UINT32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_counter_type).
pub const D3D11_COUNTER_TYPE_UINT32: CounterType = CounterType(2);
/// The header's `D3D11_COUNTER_TYPE_UINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_counter_type).
pub const D3D11_COUNTER_TYPE_UINT64: CounterType = CounterType(3);

/// The header's `D3D11_CPU_ACCESS_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_cpu_access_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CpuAccessFlag(pub i32);

/// The header's `D3D11_CPU_ACCESS_WRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_cpu_access_flag).
pub const D3D11_CPU_ACCESS_WRITE: CpuAccessFlag = CpuAccessFlag(0x10000);
/// The header's `D3D11_CPU_ACCESS_READ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_cpu_access_flag).
pub const D3D11_CPU_ACCESS_READ: CpuAccessFlag = CpuAccessFlag(0x20000);

/// The header's `D3D11_CREATE_DEVICE_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CreateDeviceFlag(pub u32);

/// The header's `D3D11_CREATE_DEVICE_SINGLETHREADED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_SINGLETHREADED: CreateDeviceFlag = CreateDeviceFlag(1);
/// The header's `D3D11_CREATE_DEVICE_DEBUG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_DEBUG: CreateDeviceFlag = CreateDeviceFlag(2);
/// The header's `D3D11_CREATE_DEVICE_SWITCH_TO_REF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_SWITCH_TO_REF: CreateDeviceFlag = CreateDeviceFlag(4);
/// The header's `D3D11_CREATE_DEVICE_PREVENT_INTERNAL_THREADING_OPTIMIZATIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_PREVENT_INTERNAL_THREADING_OPTIMIZATIONS: CreateDeviceFlag =
    CreateDeviceFlag(8);
/// The header's `D3D11_CREATE_DEVICE_BGRA_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_BGRA_SUPPORT: CreateDeviceFlag = CreateDeviceFlag(32);
/// The header's `D3D11_CREATE_DEVICE_DEBUGGABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_DEBUGGABLE: CreateDeviceFlag = CreateDeviceFlag(64);
/// The header's `D3D11_CREATE_DEVICE_PREVENT_ALTERING_LAYER_SETTINGS_FROM_REGISTRY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_PREVENT_ALTERING_LAYER_SETTINGS_FROM_REGISTRY: CreateDeviceFlag =
    CreateDeviceFlag(128);
/// The header's `D3D11_CREATE_DEVICE_DISABLE_GPU_TIMEOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_DISABLE_GPU_TIMEOUT: CreateDeviceFlag = CreateDeviceFlag(0x100);
/// The header's `D3D11_CREATE_DEVICE_VIDEO_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag).
pub const D3D11_CREATE_DEVICE_VIDEO_SUPPORT: CreateDeviceFlag = CreateDeviceFlag(0x800);

/// The header's `D3D11_CRYPTO_SESSION_KEY_EXCHANGE_FLAGS`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CryptoSessionKeyExchangeFlags(pub i32);

/// The header's `D3D11_CRYPTO_SESSION_KEY_EXCHANGE_FLAG_NONE`, [as the metadata gives it](https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31).
pub const D3D11_CRYPTO_SESSION_KEY_EXCHANGE_FLAG_NONE: CryptoSessionKeyExchangeFlags =
    CryptoSessionKeyExchangeFlags(0);

/// The header's `D3D11_CRYPTO_SESSION_STATUS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_crypto_session_status).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CryptoSessionStatus(pub i32);

/// The header's `D3D11_CRYPTO_SESSION_STATUS_OK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_crypto_session_status).
pub const D3D11_CRYPTO_SESSION_STATUS_OK: CryptoSessionStatus = CryptoSessionStatus(0);
/// The header's `D3D11_CRYPTO_SESSION_STATUS_KEY_LOST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_crypto_session_status).
pub const D3D11_CRYPTO_SESSION_STATUS_KEY_LOST: CryptoSessionStatus = CryptoSessionStatus(1);
/// The header's `D3D11_CRYPTO_SESSION_STATUS_KEY_AND_CONTENT_LOST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_crypto_session_status).
pub const D3D11_CRYPTO_SESSION_STATUS_KEY_AND_CONTENT_LOST: CryptoSessionStatus =
    CryptoSessionStatus(2);

/// The header's `D3D11_CULL_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_cull_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CullMode(pub i32);

/// The header's `D3D11_CULL_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_cull_mode).
pub const D3D11_CULL_NONE: CullMode = CullMode(1);
/// The header's `D3D11_CULL_FRONT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_cull_mode).
pub const D3D11_CULL_FRONT: CullMode = CullMode(2);
/// The header's `D3D11_CULL_BACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_cull_mode).
pub const D3D11_CULL_BACK: CullMode = CullMode(3);

/// The header's `D3D11_DEPTH_WRITE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_depth_write_mask).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DepthWriteMask(pub i32);

/// The header's `D3D11_DEPTH_WRITE_MASK_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_depth_write_mask).
pub const D3D11_DEPTH_WRITE_MASK_ZERO: DepthWriteMask = DepthWriteMask(0);
/// The header's `D3D11_DEPTH_WRITE_MASK_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_depth_write_mask).
pub const D3D11_DEPTH_WRITE_MASK_ALL: DepthWriteMask = DepthWriteMask(1);

/// The header's `D3D11_DEVICE_CONTEXT_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_device_context_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeviceContextType(pub i32);

/// The header's `D3D11_DEVICE_CONTEXT_IMMEDIATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_device_context_type).
pub const D3D11_DEVICE_CONTEXT_IMMEDIATE: DeviceContextType = DeviceContextType(0);
/// The header's `D3D11_DEVICE_CONTEXT_DEFERRED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_device_context_type).
pub const D3D11_DEVICE_CONTEXT_DEFERRED: DeviceContextType = DeviceContextType(1);

/// The header's `D3D11_DSV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DsvDimension(pub i32);

/// The header's `D3D11_DSV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_dimension).
pub const D3D11_DSV_DIMENSION_UNKNOWN: DsvDimension = DsvDimension(0);
/// The header's `D3D11_DSV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_dimension).
pub const D3D11_DSV_DIMENSION_TEXTURE1D: DsvDimension = DsvDimension(1);
/// The header's `D3D11_DSV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_dimension).
pub const D3D11_DSV_DIMENSION_TEXTURE1DARRAY: DsvDimension = DsvDimension(2);
/// The header's `D3D11_DSV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_dimension).
pub const D3D11_DSV_DIMENSION_TEXTURE2D: DsvDimension = DsvDimension(3);
/// The header's `D3D11_DSV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_dimension).
pub const D3D11_DSV_DIMENSION_TEXTURE2DARRAY: DsvDimension = DsvDimension(4);
/// The header's `D3D11_DSV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_dimension).
pub const D3D11_DSV_DIMENSION_TEXTURE2DMS: DsvDimension = DsvDimension(5);
/// The header's `D3D11_DSV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_dimension).
pub const D3D11_DSV_DIMENSION_TEXTURE2DMSARRAY: DsvDimension = DsvDimension(6);

/// The header's `D3D11_DSV_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DsvFlag(pub i32);

/// The header's `D3D11_DSV_READ_ONLY_DEPTH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_flag).
pub const D3D11_DSV_READ_ONLY_DEPTH: DsvFlag = DsvFlag(1);
/// The header's `D3D11_DSV_READ_ONLY_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_dsv_flag).
pub const D3D11_DSV_READ_ONLY_STENCIL: DsvFlag = DsvFlag(2);

/// The header's `D3D11_FEATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Feature(pub i32);

/// The header's `D3D11_FEATURE_THREADING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_THREADING: Feature = Feature(0);
/// The header's `D3D11_FEATURE_DOUBLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_DOUBLES: Feature = Feature(1);
/// The header's `D3D11_FEATURE_FORMAT_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_FORMAT_SUPPORT: Feature = Feature(2);
/// The header's `D3D11_FEATURE_FORMAT_SUPPORT2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_FORMAT_SUPPORT2: Feature = Feature(3);
/// The header's `D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS: Feature = Feature(4);
/// The header's `D3D11_FEATURE_D3D11_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D11_OPTIONS: Feature = Feature(5);
/// The header's `D3D11_FEATURE_ARCHITECTURE_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_ARCHITECTURE_INFO: Feature = Feature(6);
/// The header's `D3D11_FEATURE_D3D9_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D9_OPTIONS: Feature = Feature(7);
/// The header's `D3D11_FEATURE_SHADER_MIN_PRECISION_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_SHADER_MIN_PRECISION_SUPPORT: Feature = Feature(8);
/// The header's `D3D11_FEATURE_D3D9_SHADOW_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D9_SHADOW_SUPPORT: Feature = Feature(9);
/// The header's `D3D11_FEATURE_D3D11_OPTIONS1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D11_OPTIONS1: Feature = Feature(10);
/// The header's `D3D11_FEATURE_D3D9_SIMPLE_INSTANCING_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D9_SIMPLE_INSTANCING_SUPPORT: Feature = Feature(11);
/// The header's `D3D11_FEATURE_MARKER_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_MARKER_SUPPORT: Feature = Feature(12);
/// The header's `D3D11_FEATURE_D3D9_OPTIONS1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D9_OPTIONS1: Feature = Feature(13);
/// The header's `D3D11_FEATURE_D3D11_OPTIONS2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D11_OPTIONS2: Feature = Feature(14);
/// The header's `D3D11_FEATURE_D3D11_OPTIONS3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D11_OPTIONS3: Feature = Feature(15);
/// The header's `D3D11_FEATURE_GPU_VIRTUAL_ADDRESS_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_GPU_VIRTUAL_ADDRESS_SUPPORT: Feature = Feature(16);
/// The header's `D3D11_FEATURE_D3D11_OPTIONS4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D11_OPTIONS4: Feature = Feature(17);
/// The header's `D3D11_FEATURE_SHADER_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_SHADER_CACHE: Feature = Feature(18);
/// The header's `D3D11_FEATURE_D3D11_OPTIONS5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_D3D11_OPTIONS5: Feature = Feature(19);
/// The header's `D3D11_FEATURE_DISPLAYABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_feature).
pub const D3D11_FEATURE_DISPLAYABLE: Feature = Feature(20);

/// The header's `D3D11_FEATURE_VIDEO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_feature_video).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FeatureVideo(pub i32);

/// The header's `D3D11_FEATURE_VIDEO_DECODER_HISTOGRAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_feature_video).
pub const D3D11_FEATURE_VIDEO_DECODER_HISTOGRAM: FeatureVideo = FeatureVideo(0);

/// The header's `D3D11_FENCE_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_fence_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FenceFlag(pub i32);

/// The header's `D3D11_FENCE_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_fence_flag).
pub const D3D11_FENCE_FLAG_NONE: FenceFlag = FenceFlag(0);
/// The header's `D3D11_FENCE_FLAG_SHARED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_fence_flag).
pub const D3D11_FENCE_FLAG_SHARED: FenceFlag = FenceFlag(2);
/// The header's `D3D11_FENCE_FLAG_SHARED_CROSS_ADAPTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_fence_flag).
pub const D3D11_FENCE_FLAG_SHARED_CROSS_ADAPTER: FenceFlag = FenceFlag(4);
/// The header's `D3D11_FENCE_FLAG_NON_MONITORED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_fence_flag).
pub const D3D11_FENCE_FLAG_NON_MONITORED: FenceFlag = FenceFlag(8);

/// The header's `D3D11_FILL_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_fill_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FillMode(pub i32);

/// The header's `D3D11_FILL_WIREFRAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_fill_mode).
pub const D3D11_FILL_WIREFRAME: FillMode = FillMode(2);
/// The header's `D3D11_FILL_SOLID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_fill_mode).
pub const D3D11_FILL_SOLID: FillMode = FillMode(3);

/// The header's `D3D11_FILTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Filter(pub i32);

/// The header's `D3D11_FILTER_MIN_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MIN_MAG_MIP_POINT: Filter = Filter(0);
/// The header's `D3D11_FILTER_MIN_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MIN_MAG_POINT_MIP_LINEAR: Filter = Filter(1);
/// The header's `D3D11_FILTER_MIN_POINT_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MIN_POINT_MAG_LINEAR_MIP_POINT: Filter = Filter(4);
/// The header's `D3D11_FILTER_MIN_POINT_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MIN_POINT_MAG_MIP_LINEAR: Filter = Filter(5);
/// The header's `D3D11_FILTER_MIN_LINEAR_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MIN_LINEAR_MAG_MIP_POINT: Filter = Filter(16);
/// The header's `D3D11_FILTER_MIN_LINEAR_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MIN_LINEAR_MAG_POINT_MIP_LINEAR: Filter = Filter(17);
/// The header's `D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT: Filter = Filter(20);
/// The header's `D3D11_FILTER_MIN_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MIN_MAG_MIP_LINEAR: Filter = Filter(21);
/// The header's `D3D11_FILTER_ANISOTROPIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_ANISOTROPIC: Filter = Filter(85);
/// The header's `D3D11_FILTER_COMPARISON_MIN_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_MIN_MAG_MIP_POINT: Filter = Filter(128);
/// The header's `D3D11_FILTER_COMPARISON_MIN_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_MIN_MAG_POINT_MIP_LINEAR: Filter = Filter(129);
/// The header's `D3D11_FILTER_COMPARISON_MIN_POINT_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_MIN_POINT_MAG_LINEAR_MIP_POINT: Filter = Filter(132);
/// The header's `D3D11_FILTER_COMPARISON_MIN_POINT_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_MIN_POINT_MAG_MIP_LINEAR: Filter = Filter(133);
/// The header's `D3D11_FILTER_COMPARISON_MIN_LINEAR_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_MIN_LINEAR_MAG_MIP_POINT: Filter = Filter(144);
/// The header's `D3D11_FILTER_COMPARISON_MIN_LINEAR_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_MIN_LINEAR_MAG_POINT_MIP_LINEAR: Filter = Filter(145);
/// The header's `D3D11_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT: Filter = Filter(148);
/// The header's `D3D11_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR: Filter = Filter(149);
/// The header's `D3D11_FILTER_COMPARISON_ANISOTROPIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_COMPARISON_ANISOTROPIC: Filter = Filter(213);
/// The header's `D3D11_FILTER_MINIMUM_MIN_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_MIN_MAG_MIP_POINT: Filter = Filter(0x100);
/// The header's `D3D11_FILTER_MINIMUM_MIN_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_MIN_MAG_POINT_MIP_LINEAR: Filter = Filter(0x101);
/// The header's `D3D11_FILTER_MINIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT: Filter = Filter(0x104);
/// The header's `D3D11_FILTER_MINIMUM_MIN_POINT_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_MIN_POINT_MAG_MIP_LINEAR: Filter = Filter(0x105);
/// The header's `D3D11_FILTER_MINIMUM_MIN_LINEAR_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_MIN_LINEAR_MAG_MIP_POINT: Filter = Filter(0x110);
/// The header's `D3D11_FILTER_MINIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR: Filter = Filter(0x111);
/// The header's `D3D11_FILTER_MINIMUM_MIN_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_MIN_MAG_LINEAR_MIP_POINT: Filter = Filter(0x114);
/// The header's `D3D11_FILTER_MINIMUM_MIN_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_MIN_MAG_MIP_LINEAR: Filter = Filter(0x115);
/// The header's `D3D11_FILTER_MINIMUM_ANISOTROPIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MINIMUM_ANISOTROPIC: Filter = Filter(0x155);
/// The header's `D3D11_FILTER_MAXIMUM_MIN_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_MIN_MAG_MIP_POINT: Filter = Filter(0x180);
/// The header's `D3D11_FILTER_MAXIMUM_MIN_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_MIN_MAG_POINT_MIP_LINEAR: Filter = Filter(0x181);
/// The header's `D3D11_FILTER_MAXIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_MIN_POINT_MAG_LINEAR_MIP_POINT: Filter = Filter(0x184);
/// The header's `D3D11_FILTER_MAXIMUM_MIN_POINT_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_MIN_POINT_MAG_MIP_LINEAR: Filter = Filter(0x185);
/// The header's `D3D11_FILTER_MAXIMUM_MIN_LINEAR_MAG_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_MIN_LINEAR_MAG_MIP_POINT: Filter = Filter(0x190);
/// The header's `D3D11_FILTER_MAXIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_MIN_LINEAR_MAG_POINT_MIP_LINEAR: Filter = Filter(0x191);
/// The header's `D3D11_FILTER_MAXIMUM_MIN_MAG_LINEAR_MIP_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_MIN_MAG_LINEAR_MIP_POINT: Filter = Filter(0x194);
/// The header's `D3D11_FILTER_MAXIMUM_MIN_MAG_MIP_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_MIN_MAG_MIP_LINEAR: Filter = Filter(0x195);
/// The header's `D3D11_FILTER_MAXIMUM_ANISOTROPIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter).
pub const D3D11_FILTER_MAXIMUM_ANISOTROPIC: Filter = Filter(0x1d5);

/// The header's `D3D11_FILTER_REDUCTION_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter_reduction_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FilterReductionType(pub i32);

/// The header's `D3D11_FILTER_REDUCTION_TYPE_STANDARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter_reduction_type).
pub const D3D11_FILTER_REDUCTION_TYPE_STANDARD: FilterReductionType = FilterReductionType(0);
/// The header's `D3D11_FILTER_REDUCTION_TYPE_COMPARISON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter_reduction_type).
pub const D3D11_FILTER_REDUCTION_TYPE_COMPARISON: FilterReductionType = FilterReductionType(1);
/// The header's `D3D11_FILTER_REDUCTION_TYPE_MINIMUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter_reduction_type).
pub const D3D11_FILTER_REDUCTION_TYPE_MINIMUM: FilterReductionType = FilterReductionType(2);
/// The header's `D3D11_FILTER_REDUCTION_TYPE_MAXIMUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter_reduction_type).
pub const D3D11_FILTER_REDUCTION_TYPE_MAXIMUM: FilterReductionType = FilterReductionType(3);

/// The header's `D3D11_FILTER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FilterType(pub i32);

/// The header's `D3D11_FILTER_TYPE_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter_type).
pub const D3D11_FILTER_TYPE_POINT: FilterType = FilterType(0);
/// The header's `D3D11_FILTER_TYPE_LINEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_filter_type).
pub const D3D11_FILTER_TYPE_LINEAR: FilterType = FilterType(1);

/// The header's `D3D11_FORMAT_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatSupport(pub i32);

/// The header's `D3D11_FORMAT_SUPPORT_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_BUFFER: FormatSupport = FormatSupport(1);
/// The header's `D3D11_FORMAT_SUPPORT_IA_VERTEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_IA_VERTEX_BUFFER: FormatSupport = FormatSupport(2);
/// The header's `D3D11_FORMAT_SUPPORT_IA_INDEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_IA_INDEX_BUFFER: FormatSupport = FormatSupport(4);
/// The header's `D3D11_FORMAT_SUPPORT_SO_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_SO_BUFFER: FormatSupport = FormatSupport(8);
/// The header's `D3D11_FORMAT_SUPPORT_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_TEXTURE1D: FormatSupport = FormatSupport(16);
/// The header's `D3D11_FORMAT_SUPPORT_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_TEXTURE2D: FormatSupport = FormatSupport(32);
/// The header's `D3D11_FORMAT_SUPPORT_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_TEXTURE3D: FormatSupport = FormatSupport(64);
/// The header's `D3D11_FORMAT_SUPPORT_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_TEXTURECUBE: FormatSupport = FormatSupport(128);
/// The header's `D3D11_FORMAT_SUPPORT_SHADER_LOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_SHADER_LOAD: FormatSupport = FormatSupport(0x100);
/// The header's `D3D11_FORMAT_SUPPORT_SHADER_SAMPLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_SHADER_SAMPLE: FormatSupport = FormatSupport(0x200);
/// The header's `D3D11_FORMAT_SUPPORT_SHADER_SAMPLE_COMPARISON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_SHADER_SAMPLE_COMPARISON: FormatSupport = FormatSupport(0x400);
/// The header's `D3D11_FORMAT_SUPPORT_SHADER_SAMPLE_MONO_TEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_SHADER_SAMPLE_MONO_TEXT: FormatSupport = FormatSupport(0x800);
/// The header's `D3D11_FORMAT_SUPPORT_MIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_MIP: FormatSupport = FormatSupport(0x1000);
/// The header's `D3D11_FORMAT_SUPPORT_MIP_AUTOGEN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_MIP_AUTOGEN: FormatSupport = FormatSupport(0x2000);
/// The header's `D3D11_FORMAT_SUPPORT_RENDER_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_RENDER_TARGET: FormatSupport = FormatSupport(0x4000);
/// The header's `D3D11_FORMAT_SUPPORT_BLENDABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_BLENDABLE: FormatSupport = FormatSupport(0x8000);
/// The header's `D3D11_FORMAT_SUPPORT_DEPTH_STENCIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_DEPTH_STENCIL: FormatSupport = FormatSupport(0x10000);
/// The header's `D3D11_FORMAT_SUPPORT_CPU_LOCKABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_CPU_LOCKABLE: FormatSupport = FormatSupport(0x20000);
/// The header's `D3D11_FORMAT_SUPPORT_MULTISAMPLE_RESOLVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_MULTISAMPLE_RESOLVE: FormatSupport = FormatSupport(0x40000);
/// The header's `D3D11_FORMAT_SUPPORT_DISPLAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_DISPLAY: FormatSupport = FormatSupport(0x80000);
/// The header's `D3D11_FORMAT_SUPPORT_CAST_WITHIN_BIT_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_CAST_WITHIN_BIT_LAYOUT: FormatSupport = FormatSupport(0x100000);
/// The header's `D3D11_FORMAT_SUPPORT_MULTISAMPLE_RENDERTARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_MULTISAMPLE_RENDERTARGET: FormatSupport = FormatSupport(0x200000);
/// The header's `D3D11_FORMAT_SUPPORT_MULTISAMPLE_LOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_MULTISAMPLE_LOAD: FormatSupport = FormatSupport(0x400000);
/// The header's `D3D11_FORMAT_SUPPORT_SHADER_GATHER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_SHADER_GATHER: FormatSupport = FormatSupport(0x800000);
/// The header's `D3D11_FORMAT_SUPPORT_BACK_BUFFER_CAST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_BACK_BUFFER_CAST: FormatSupport = FormatSupport(0x1000000);
/// The header's `D3D11_FORMAT_SUPPORT_TYPED_UNORDERED_ACCESS_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_TYPED_UNORDERED_ACCESS_VIEW: FormatSupport =
    FormatSupport(0x2000000);
/// The header's `D3D11_FORMAT_SUPPORT_SHADER_GATHER_COMPARISON`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_SHADER_GATHER_COMPARISON: FormatSupport = FormatSupport(0x4000000);
/// The header's `D3D11_FORMAT_SUPPORT_DECODER_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_DECODER_OUTPUT: FormatSupport = FormatSupport(0x8000000);
/// The header's `D3D11_FORMAT_SUPPORT_VIDEO_PROCESSOR_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_VIDEO_PROCESSOR_OUTPUT: FormatSupport = FormatSupport(0x10000000);
/// The header's `D3D11_FORMAT_SUPPORT_VIDEO_PROCESSOR_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_VIDEO_PROCESSOR_INPUT: FormatSupport = FormatSupport(0x20000000);
/// The header's `D3D11_FORMAT_SUPPORT_VIDEO_ENCODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support).
pub const D3D11_FORMAT_SUPPORT_VIDEO_ENCODER: FormatSupport = FormatSupport(0x40000000);

/// The header's `D3D11_FORMAT_SUPPORT2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatSupport2(pub i32);

/// The header's `D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_ADD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_ADD: FormatSupport2 = FormatSupport2(1);
/// The header's `D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_BITWISE_OPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_BITWISE_OPS: FormatSupport2 = FormatSupport2(2);
/// The header's `D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE: FormatSupport2 =
    FormatSupport2(4);
/// The header's `D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_EXCHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_EXCHANGE: FormatSupport2 = FormatSupport2(8);
/// The header's `D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_SIGNED_MIN_OR_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_SIGNED_MIN_OR_MAX: FormatSupport2 = FormatSupport2(16);
/// The header's `D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_UNSIGNED_MIN_OR_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_UAV_ATOMIC_UNSIGNED_MIN_OR_MAX: FormatSupport2 = FormatSupport2(32);
/// The header's `D3D11_FORMAT_SUPPORT2_UAV_TYPED_LOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_UAV_TYPED_LOAD: FormatSupport2 = FormatSupport2(64);
/// The header's `D3D11_FORMAT_SUPPORT2_UAV_TYPED_STORE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_UAV_TYPED_STORE: FormatSupport2 = FormatSupport2(128);
/// The header's `D3D11_FORMAT_SUPPORT2_OUTPUT_MERGER_LOGIC_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_OUTPUT_MERGER_LOGIC_OP: FormatSupport2 = FormatSupport2(0x100);
/// The header's `D3D11_FORMAT_SUPPORT2_TILED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_TILED: FormatSupport2 = FormatSupport2(0x200);
/// The header's `D3D11_FORMAT_SUPPORT2_SHAREABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_SHAREABLE: FormatSupport2 = FormatSupport2(0x400);
/// The header's `D3D11_FORMAT_SUPPORT2_MULTIPLANE_OVERLAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_MULTIPLANE_OVERLAY: FormatSupport2 = FormatSupport2(0x4000);
/// The header's `D3D11_FORMAT_SUPPORT2_DISPLAYABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_format_support2).
pub const D3D11_FORMAT_SUPPORT2_DISPLAYABLE: FormatSupport2 = FormatSupport2(0x10000);

/// The header's `D3D11_INPUT_CLASSIFICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_input_classification).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InputClassification(pub i32);

/// The header's `D3D11_INPUT_PER_VERTEX_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_input_classification).
pub const D3D11_INPUT_PER_VERTEX_DATA: InputClassification = InputClassification(0);
/// The header's `D3D11_INPUT_PER_INSTANCE_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_input_classification).
pub const D3D11_INPUT_PER_INSTANCE_DATA: InputClassification = InputClassification(1);

/// The header's `D3D11_LOGIC_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LogicOp(pub i32);

/// The header's `D3D11_LOGIC_OP_CLEAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_CLEAR: LogicOp = LogicOp(0);
/// The header's `D3D11_LOGIC_OP_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_SET: LogicOp = LogicOp(1);
/// The header's `D3D11_LOGIC_OP_COPY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_COPY: LogicOp = LogicOp(2);
/// The header's `D3D11_LOGIC_OP_COPY_INVERTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_COPY_INVERTED: LogicOp = LogicOp(3);
/// The header's `D3D11_LOGIC_OP_NOOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_NOOP: LogicOp = LogicOp(4);
/// The header's `D3D11_LOGIC_OP_INVERT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_INVERT: LogicOp = LogicOp(5);
/// The header's `D3D11_LOGIC_OP_AND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_AND: LogicOp = LogicOp(6);
/// The header's `D3D11_LOGIC_OP_NAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_NAND: LogicOp = LogicOp(7);
/// The header's `D3D11_LOGIC_OP_OR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_OR: LogicOp = LogicOp(8);
/// The header's `D3D11_LOGIC_OP_NOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_NOR: LogicOp = LogicOp(9);
/// The header's `D3D11_LOGIC_OP_XOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_XOR: LogicOp = LogicOp(10);
/// The header's `D3D11_LOGIC_OP_EQUIV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_EQUIV: LogicOp = LogicOp(11);
/// The header's `D3D11_LOGIC_OP_AND_REVERSE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_AND_REVERSE: LogicOp = LogicOp(12);
/// The header's `D3D11_LOGIC_OP_AND_INVERTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_AND_INVERTED: LogicOp = LogicOp(13);
/// The header's `D3D11_LOGIC_OP_OR_REVERSE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_OR_REVERSE: LogicOp = LogicOp(14);
/// The header's `D3D11_LOGIC_OP_OR_INVERTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_logic_op).
pub const D3D11_LOGIC_OP_OR_INVERTED: LogicOp = LogicOp(15);

/// The header's `D3D11_MAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Map(pub i32);

/// The header's `D3D11_MAP_READ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map).
pub const D3D11_MAP_READ: Map = Map(1);
/// The header's `D3D11_MAP_WRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map).
pub const D3D11_MAP_WRITE: Map = Map(2);
/// The header's `D3D11_MAP_READ_WRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map).
pub const D3D11_MAP_READ_WRITE: Map = Map(3);
/// The header's `D3D11_MAP_WRITE_DISCARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map).
pub const D3D11_MAP_WRITE_DISCARD: Map = Map(4);
/// The header's `D3D11_MAP_WRITE_NO_OVERWRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map).
pub const D3D11_MAP_WRITE_NO_OVERWRITE: Map = Map(5);

/// The header's `D3D11_MAP_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MapFlag(pub i32);

/// The header's `D3D11_MAP_FLAG_DO_NOT_WAIT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map_flag).
pub const D3D11_MAP_FLAG_DO_NOT_WAIT: MapFlag = MapFlag(0x100000);

/// The header's `D3D11_MESSAGE_CATEGORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MessageCategory(pub i32);

/// The header's `D3D11_MESSAGE_CATEGORY_APPLICATION_DEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_APPLICATION_DEFINED: MessageCategory = MessageCategory(0);
/// The header's `D3D11_MESSAGE_CATEGORY_MISCELLANEOUS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_MISCELLANEOUS: MessageCategory = MessageCategory(1);
/// The header's `D3D11_MESSAGE_CATEGORY_INITIALIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_INITIALIZATION: MessageCategory = MessageCategory(2);
/// The header's `D3D11_MESSAGE_CATEGORY_CLEANUP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_CLEANUP: MessageCategory = MessageCategory(3);
/// The header's `D3D11_MESSAGE_CATEGORY_COMPILATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_COMPILATION: MessageCategory = MessageCategory(4);
/// The header's `D3D11_MESSAGE_CATEGORY_STATE_CREATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_STATE_CREATION: MessageCategory = MessageCategory(5);
/// The header's `D3D11_MESSAGE_CATEGORY_STATE_SETTING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_STATE_SETTING: MessageCategory = MessageCategory(6);
/// The header's `D3D11_MESSAGE_CATEGORY_STATE_GETTING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_STATE_GETTING: MessageCategory = MessageCategory(7);
/// The header's `D3D11_MESSAGE_CATEGORY_RESOURCE_MANIPULATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_RESOURCE_MANIPULATION: MessageCategory = MessageCategory(8);
/// The header's `D3D11_MESSAGE_CATEGORY_EXECUTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_EXECUTION: MessageCategory = MessageCategory(9);
/// The header's `D3D11_MESSAGE_CATEGORY_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_category).
pub const D3D11_MESSAGE_CATEGORY_SHADER: MessageCategory = MessageCategory(10);

/// The header's `D3D11_MESSAGE_ID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MessageId(pub i32);

/// The header's `D3D11_MESSAGE_ID_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UNKNOWN: MessageId = MessageId(0);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_HAZARD: MessageId = MessageId(1);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETINDEXBUFFER_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETINDEXBUFFER_HAZARD: MessageId = MessageId(2);
/// The header's `D3D11_MESSAGE_ID_DEVICE_VSSETSHADERRESOURCES_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_VSSETSHADERRESOURCES_HAZARD: MessageId = MessageId(3);
/// The header's `D3D11_MESSAGE_ID_DEVICE_VSSETCONSTANTBUFFERS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_VSSETCONSTANTBUFFERS_HAZARD: MessageId = MessageId(4);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GSSETSHADERRESOURCES_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GSSETSHADERRESOURCES_HAZARD: MessageId = MessageId(5);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GSSETCONSTANTBUFFERS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GSSETCONSTANTBUFFERS_HAZARD: MessageId = MessageId(6);
/// The header's `D3D11_MESSAGE_ID_DEVICE_PSSETSHADERRESOURCES_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_PSSETSHADERRESOURCES_HAZARD: MessageId = MessageId(7);
/// The header's `D3D11_MESSAGE_ID_DEVICE_PSSETCONSTANTBUFFERS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_PSSETCONSTANTBUFFERS_HAZARD: MessageId = MessageId(8);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETS_HAZARD: MessageId = MessageId(9);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SOSETTARGETS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SOSETTARGETS_HAZARD: MessageId = MessageId(10);
/// The header's `D3D11_MESSAGE_ID_STRING_FROM_APPLICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_STRING_FROM_APPLICATION: MessageId = MessageId(11);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_THIS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_THIS: MessageId = MessageId(12);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER1: MessageId = MessageId(13);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER2: MessageId = MessageId(14);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER3: MessageId = MessageId(15);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER4: MessageId = MessageId(16);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER5: MessageId = MessageId(17);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER6: MessageId = MessageId(18);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER7: MessageId = MessageId(19);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER8: MessageId = MessageId(20);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER9`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER9: MessageId = MessageId(21);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER10: MessageId = MessageId(22);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER11`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER11: MessageId = MessageId(23);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER12`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER12: MessageId = MessageId(24);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER13`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER13: MessageId = MessageId(25);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER14`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER14: MessageId = MessageId(26);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_PARAMETER15`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_PARAMETER15: MessageId = MessageId(27);
/// The header's `D3D11_MESSAGE_ID_CORRUPTED_MULTITHREADING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CORRUPTED_MULTITHREADING: MessageId = MessageId(28);
/// The header's `D3D11_MESSAGE_ID_MESSAGE_REPORTING_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_MESSAGE_REPORTING_OUTOFMEMORY: MessageId = MessageId(29);
/// The header's `D3D11_MESSAGE_ID_IASETINPUTLAYOUT_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_IASETINPUTLAYOUT_UNBINDDELETINGOBJECT: MessageId = MessageId(30);
/// The header's `D3D11_MESSAGE_ID_IASETVERTEXBUFFERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_IASETVERTEXBUFFERS_UNBINDDELETINGOBJECT: MessageId = MessageId(31);
/// The header's `D3D11_MESSAGE_ID_IASETINDEXBUFFER_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_IASETINDEXBUFFER_UNBINDDELETINGOBJECT: MessageId = MessageId(32);
/// The header's `D3D11_MESSAGE_ID_VSSETSHADER_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSETSHADER_UNBINDDELETINGOBJECT: MessageId = MessageId(33);
/// The header's `D3D11_MESSAGE_ID_VSSETSHADERRESOURCES_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSETSHADERRESOURCES_UNBINDDELETINGOBJECT: MessageId = MessageId(34);
/// The header's `D3D11_MESSAGE_ID_VSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT: MessageId = MessageId(35);
/// The header's `D3D11_MESSAGE_ID_VSSETSAMPLERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSETSAMPLERS_UNBINDDELETINGOBJECT: MessageId = MessageId(36);
/// The header's `D3D11_MESSAGE_ID_GSSETSHADER_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GSSETSHADER_UNBINDDELETINGOBJECT: MessageId = MessageId(37);
/// The header's `D3D11_MESSAGE_ID_GSSETSHADERRESOURCES_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GSSETSHADERRESOURCES_UNBINDDELETINGOBJECT: MessageId = MessageId(38);
/// The header's `D3D11_MESSAGE_ID_GSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT: MessageId = MessageId(39);
/// The header's `D3D11_MESSAGE_ID_GSSETSAMPLERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GSSETSAMPLERS_UNBINDDELETINGOBJECT: MessageId = MessageId(40);
/// The header's `D3D11_MESSAGE_ID_SOSETTARGETS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SOSETTARGETS_UNBINDDELETINGOBJECT: MessageId = MessageId(41);
/// The header's `D3D11_MESSAGE_ID_PSSETSHADER_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PSSETSHADER_UNBINDDELETINGOBJECT: MessageId = MessageId(42);
/// The header's `D3D11_MESSAGE_ID_PSSETSHADERRESOURCES_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PSSETSHADERRESOURCES_UNBINDDELETINGOBJECT: MessageId = MessageId(43);
/// The header's `D3D11_MESSAGE_ID_PSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT: MessageId = MessageId(44);
/// The header's `D3D11_MESSAGE_ID_PSSETSAMPLERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PSSETSAMPLERS_UNBINDDELETINGOBJECT: MessageId = MessageId(45);
/// The header's `D3D11_MESSAGE_ID_RSSETSTATE_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RSSETSTATE_UNBINDDELETINGOBJECT: MessageId = MessageId(46);
/// The header's `D3D11_MESSAGE_ID_OMSETBLENDSTATE_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OMSETBLENDSTATE_UNBINDDELETINGOBJECT: MessageId = MessageId(47);
/// The header's `D3D11_MESSAGE_ID_OMSETDEPTHSTENCILSTATE_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OMSETDEPTHSTENCILSTATE_UNBINDDELETINGOBJECT: MessageId = MessageId(48);
/// The header's `D3D11_MESSAGE_ID_OMSETRENDERTARGETS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OMSETRENDERTARGETS_UNBINDDELETINGOBJECT: MessageId = MessageId(49);
/// The header's `D3D11_MESSAGE_ID_SETPREDICATION_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETPREDICATION_UNBINDDELETINGOBJECT: MessageId = MessageId(50);
/// The header's `D3D11_MESSAGE_ID_GETPRIVATEDATA_MOREDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETPRIVATEDATA_MOREDATA: MessageId = MessageId(51);
/// The header's `D3D11_MESSAGE_ID_SETPRIVATEDATA_INVALIDFREEDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETPRIVATEDATA_INVALIDFREEDATA: MessageId = MessageId(52);
/// The header's `D3D11_MESSAGE_ID_SETPRIVATEDATA_INVALIDIUNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETPRIVATEDATA_INVALIDIUNKNOWN: MessageId = MessageId(53);
/// The header's `D3D11_MESSAGE_ID_SETPRIVATEDATA_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETPRIVATEDATA_INVALIDFLAGS: MessageId = MessageId(54);
/// The header's `D3D11_MESSAGE_ID_SETPRIVATEDATA_CHANGINGPARAMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETPRIVATEDATA_CHANGINGPARAMS: MessageId = MessageId(55);
/// The header's `D3D11_MESSAGE_ID_SETPRIVATEDATA_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETPRIVATEDATA_OUTOFMEMORY: MessageId = MessageId(56);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDFORMAT: MessageId = MessageId(57);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDSAMPLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDSAMPLES: MessageId = MessageId(58);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDUSAGE: MessageId = MessageId(59);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDBINDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDBINDFLAGS: MessageId = MessageId(60);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDCPUACCESSFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDCPUACCESSFLAGS: MessageId = MessageId(61);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_UNRECOGNIZEDMISCFLAGS: MessageId = MessageId(62);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDCPUACCESSFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDCPUACCESSFLAGS: MessageId = MessageId(63);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDBINDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDBINDFLAGS: MessageId = MessageId(64);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDINITIALDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDINITIALDATA: MessageId = MessageId(65);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDDIMENSIONS: MessageId = MessageId(66);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDMIPLEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDMIPLEVELS: MessageId = MessageId(67);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDMISCFLAGS: MessageId = MessageId(68);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDARG_RETURN: MessageId = MessageId(69);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_OUTOFMEMORY_RETURN: MessageId = MessageId(70);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_NULLDESC: MessageId = MessageId(71);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDCONSTANTBUFFERBINDINGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDCONSTANTBUFFERBINDINGS: MessageId = MessageId(72);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_LARGEALLOCATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_LARGEALLOCATION: MessageId = MessageId(73);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDFORMAT: MessageId = MessageId(74);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_UNSUPPORTEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_UNSUPPORTEDFORMAT: MessageId = MessageId(75);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDSAMPLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDSAMPLES: MessageId = MessageId(76);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDUSAGE: MessageId = MessageId(77);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDBINDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDBINDFLAGS: MessageId = MessageId(78);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDCPUACCESSFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDCPUACCESSFLAGS: MessageId = MessageId(79);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_UNRECOGNIZEDMISCFLAGS: MessageId = MessageId(80);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDCPUACCESSFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDCPUACCESSFLAGS: MessageId = MessageId(81);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDBINDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDBINDFLAGS: MessageId = MessageId(82);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDINITIALDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDINITIALDATA: MessageId = MessageId(83);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDDIMENSIONS: MessageId = MessageId(84);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDMIPLEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDMIPLEVELS: MessageId = MessageId(85);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDMISCFLAGS: MessageId = MessageId(86);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDARG_RETURN: MessageId = MessageId(87);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_OUTOFMEMORY_RETURN: MessageId = MessageId(88);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_NULLDESC: MessageId = MessageId(89);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_LARGEALLOCATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_LARGEALLOCATION: MessageId = MessageId(90);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDFORMAT: MessageId = MessageId(91);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_UNSUPPORTEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_UNSUPPORTEDFORMAT: MessageId = MessageId(92);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDSAMPLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDSAMPLES: MessageId = MessageId(93);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDUSAGE: MessageId = MessageId(94);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDBINDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDBINDFLAGS: MessageId = MessageId(95);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDCPUACCESSFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDCPUACCESSFLAGS: MessageId = MessageId(96);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_UNRECOGNIZEDMISCFLAGS: MessageId = MessageId(97);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDCPUACCESSFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDCPUACCESSFLAGS: MessageId = MessageId(98);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDBINDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDBINDFLAGS: MessageId = MessageId(99);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDINITIALDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDINITIALDATA: MessageId = MessageId(100);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDDIMENSIONS: MessageId = MessageId(101);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDMIPLEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDMIPLEVELS: MessageId = MessageId(102);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDMISCFLAGS: MessageId = MessageId(103);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDARG_RETURN: MessageId = MessageId(104);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_OUTOFMEMORY_RETURN: MessageId = MessageId(105);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_NULLDESC: MessageId = MessageId(106);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_LARGEALLOCATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_LARGEALLOCATION: MessageId = MessageId(107);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDFORMAT: MessageId = MessageId(108);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_UNSUPPORTEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_UNSUPPORTEDFORMAT: MessageId = MessageId(109);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDSAMPLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDSAMPLES: MessageId = MessageId(110);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDUSAGE: MessageId = MessageId(111);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDBINDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDBINDFLAGS: MessageId = MessageId(112);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDCPUACCESSFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDCPUACCESSFLAGS: MessageId = MessageId(113);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_UNRECOGNIZEDMISCFLAGS: MessageId = MessageId(114);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDCPUACCESSFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDCPUACCESSFLAGS: MessageId = MessageId(115);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDBINDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDBINDFLAGS: MessageId = MessageId(116);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDINITIALDATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDINITIALDATA: MessageId = MessageId(117);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDDIMENSIONS: MessageId = MessageId(118);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDMIPLEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDMIPLEVELS: MessageId = MessageId(119);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDMISCFLAGS: MessageId = MessageId(120);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_INVALIDARG_RETURN: MessageId = MessageId(121);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_OUTOFMEMORY_RETURN: MessageId = MessageId(122);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_NULLDESC: MessageId = MessageId(123);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE3D_LARGEALLOCATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE3D_LARGEALLOCATION: MessageId = MessageId(124);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_UNRECOGNIZEDFORMAT: MessageId = MessageId(125);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDESC: MessageId = MessageId(126);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDFORMAT: MessageId = MessageId(127);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDIMENSIONS: MessageId = MessageId(128);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDRESOURCE: MessageId = MessageId(129);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_TOOMANYOBJECTS: MessageId = MessageId(130);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDARG_RETURN: MessageId = MessageId(131);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_OUTOFMEMORY_RETURN: MessageId = MessageId(132);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_UNRECOGNIZEDFORMAT: MessageId = MessageId(133);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_UNSUPPORTEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_UNSUPPORTEDFORMAT: MessageId = MessageId(134);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDESC: MessageId = MessageId(135);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDFORMAT: MessageId = MessageId(136);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDIMENSIONS: MessageId = MessageId(137);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDRESOURCE: MessageId = MessageId(138);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_TOOMANYOBJECTS: MessageId = MessageId(139);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDARG_RETURN: MessageId = MessageId(140);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_OUTOFMEMORY_RETURN: MessageId = MessageId(141);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_UNRECOGNIZEDFORMAT: MessageId = MessageId(142);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDDESC: MessageId = MessageId(143);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDFORMAT: MessageId = MessageId(144);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDDIMENSIONS: MessageId = MessageId(145);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDRESOURCE: MessageId = MessageId(146);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_TOOMANYOBJECTS: MessageId = MessageId(147);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDARG_RETURN: MessageId = MessageId(148);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_OUTOFMEMORY_RETURN: MessageId = MessageId(149);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_OUTOFMEMORY: MessageId = MessageId(150);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_TOOMANYELEMENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_TOOMANYELEMENTS: MessageId = MessageId(151);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDFORMAT: MessageId = MessageId(152);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INCOMPATIBLEFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INCOMPATIBLEFORMAT: MessageId = MessageId(153);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSLOT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSLOT: MessageId = MessageId(154);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDINPUTSLOTCLASS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDINPUTSLOTCLASS: MessageId = MessageId(155);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_STEPRATESLOTCLASSMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_STEPRATESLOTCLASSMISMATCH: MessageId = MessageId(156);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSLOTCLASSCHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSLOTCLASSCHANGE: MessageId = MessageId(157);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSTEPRATECHANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDSTEPRATECHANGE: MessageId = MessageId(158);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDALIGNMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_INVALIDALIGNMENT: MessageId = MessageId(159);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_DUPLICATESEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_DUPLICATESEMANTIC: MessageId = MessageId(160);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_UNPARSEABLEINPUTSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_UNPARSEABLEINPUTSIGNATURE: MessageId = MessageId(161);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_NULLSEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_NULLSEMANTIC: MessageId = MessageId(162);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_MISSINGELEMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_MISSINGELEMENT: MessageId = MessageId(163);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_NULLDESC: MessageId = MessageId(164);
/// The header's `D3D11_MESSAGE_ID_CREATEVERTEXSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVERTEXSHADER_OUTOFMEMORY: MessageId = MessageId(165);
/// The header's `D3D11_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(166);
/// The header's `D3D11_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDSHADERTYPE: MessageId = MessageId(167);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADER_OUTOFMEMORY: MessageId = MessageId(168);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(169);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDSHADERTYPE: MessageId = MessageId(170);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTOFMEMORY: MessageId =
    MessageId(171);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSHADERBYTECODE: MessageId =
    MessageId(172);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSHADERTYPE: MessageId =
    MessageId(173);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMENTRIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMENTRIES: MessageId =
    MessageId(174);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTPUTSTREAMSTRIDEUNUSED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTPUTSTREAMSTRIDEUNUSED:
    MessageId = MessageId(175);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDDECL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDDECL: MessageId =
    MessageId(176);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_EXPECTEDDECL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_EXPECTEDDECL: MessageId =
    MessageId(177);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTPUTSLOT0EXPECTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_OUTPUTSLOT0EXPECTED: MessageId =
    MessageId(178);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDOUTPUTSLOT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDOUTPUTSLOT: MessageId =
    MessageId(179);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_ONLYONEELEMENTPERSLOT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_ONLYONEELEMENTPERSLOT: MessageId =
    MessageId(180);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDCOMPONENTCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDCOMPONENTCOUNT: MessageId =
    MessageId(181);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTARTCOMPONENTANDCOMPONENTCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTARTCOMPONENTANDCOMPONENTCOUNT: MessageId = MessageId(182);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDGAPDEFINITION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDGAPDEFINITION: MessageId =
    MessageId(183);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_REPEATEDOUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_REPEATEDOUTPUT: MessageId =
    MessageId(184);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDOUTPUTSTREAMSTRIDE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDOUTPUTSTREAMSTRIDE:
    MessageId = MessageId(185);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MISSINGSEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MISSINGSEMANTIC: MessageId =
    MessageId(186);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MASKMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MASKMISMATCH: MessageId =
    MessageId(187);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_CANTHAVEONLYGAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_CANTHAVEONLYGAPS: MessageId =
    MessageId(188);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DECLTOOCOMPLEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DECLTOOCOMPLEX: MessageId =
    MessageId(189);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MISSINGOUTPUTSIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_MISSINGOUTPUTSIGNATURE: MessageId =
    MessageId(190);
/// The header's `D3D11_MESSAGE_ID_CREATEPIXELSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEPIXELSHADER_OUTOFMEMORY: MessageId = MessageId(191);
/// The header's `D3D11_MESSAGE_ID_CREATEPIXELSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEPIXELSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(192);
/// The header's `D3D11_MESSAGE_ID_CREATEPIXELSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEPIXELSHADER_INVALIDSHADERTYPE: MessageId = MessageId(193);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDFILLMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDFILLMODE: MessageId = MessageId(194);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDCULLMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDCULLMODE: MessageId = MessageId(195);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDDEPTHBIASCLAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDDEPTHBIASCLAMP: MessageId = MessageId(196);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDSLOPESCALEDDEPTHBIAS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDSLOPESCALEDDEPTHBIAS: MessageId =
    MessageId(197);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_TOOMANYOBJECTS: MessageId = MessageId(198);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_NULLDESC: MessageId = MessageId(199);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDDEPTHWRITEMASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDDEPTHWRITEMASK: MessageId =
    MessageId(200);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDDEPTHFUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDDEPTHFUNC: MessageId = MessageId(201);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILFAILOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILFAILOP: MessageId =
    MessageId(202);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILZFAILOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILZFAILOP: MessageId =
    MessageId(203);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILPASSOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILPASSOP: MessageId =
    MessageId(204);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILFUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDFRONTFACESTENCILFUNC: MessageId =
    MessageId(205);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILFAILOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILFAILOP: MessageId =
    MessageId(206);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILZFAILOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILZFAILOP: MessageId =
    MessageId(207);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILPASSOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILPASSOP: MessageId =
    MessageId(208);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILFUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_INVALIDBACKFACESTENCILFUNC: MessageId =
    MessageId(209);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_TOOMANYOBJECTS: MessageId = MessageId(210);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_NULLDESC: MessageId = MessageId(211);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDSRCBLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDSRCBLEND: MessageId = MessageId(212);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDDESTBLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDDESTBLEND: MessageId = MessageId(213);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDBLENDOP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDBLENDOP: MessageId = MessageId(214);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDSRCBLENDALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDSRCBLENDALPHA: MessageId = MessageId(215);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDDESTBLENDALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDDESTBLENDALPHA: MessageId = MessageId(216);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDBLENDOPALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDBLENDOPALPHA: MessageId = MessageId(217);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDRENDERTARGETWRITEMASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDRENDERTARGETWRITEMASK: MessageId =
    MessageId(218);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_TOOMANYOBJECTS: MessageId = MessageId(219);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_NULLDESC: MessageId = MessageId(220);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDFILTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDFILTER: MessageId = MessageId(221);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDADDRESSU`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDADDRESSU: MessageId = MessageId(222);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDADDRESSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDADDRESSV: MessageId = MessageId(223);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDADDRESSW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDADDRESSW: MessageId = MessageId(224);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDMIPLODBIAS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDMIPLODBIAS: MessageId = MessageId(225);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDMAXANISOTROPY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDMAXANISOTROPY: MessageId = MessageId(226);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDCOMPARISONFUNC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDCOMPARISONFUNC: MessageId = MessageId(227);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDMINLOD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDMINLOD: MessageId = MessageId(228);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDMAXLOD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_INVALIDMAXLOD: MessageId = MessageId(229);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_TOOMANYOBJECTS: MessageId = MessageId(230);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_NULLDESC: MessageId = MessageId(231);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_INVALIDQUERY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_INVALIDQUERY: MessageId = MessageId(232);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_INVALIDMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_INVALIDMISCFLAGS: MessageId = MessageId(233);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_UNEXPECTEDMISCFLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_UNEXPECTEDMISCFLAG: MessageId = MessageId(234);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_NULLDESC: MessageId = MessageId(235);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETPRIMITIVETOPOLOGY_TOPOLOGY_UNRECOGNIZED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETPRIMITIVETOPOLOGY_TOPOLOGY_UNRECOGNIZED: MessageId =
    MessageId(236);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETPRIMITIVETOPOLOGY_TOPOLOGY_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETPRIMITIVETOPOLOGY_TOPOLOGY_UNDEFINED: MessageId =
    MessageId(237);
/// The header's `D3D11_MESSAGE_ID_IASETVERTEXBUFFERS_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_IASETVERTEXBUFFERS_INVALIDBUFFER: MessageId = MessageId(238);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_OFFSET_TOO_LARGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_OFFSET_TOO_LARGE: MessageId = MessageId(239);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_BUFFERS_EMPTY: MessageId = MessageId(240);
/// The header's `D3D11_MESSAGE_ID_IASETINDEXBUFFER_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_IASETINDEXBUFFER_INVALIDBUFFER: MessageId = MessageId(241);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETINDEXBUFFER_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETINDEXBUFFER_FORMAT_INVALID: MessageId = MessageId(242);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETINDEXBUFFER_OFFSET_TOO_LARGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETINDEXBUFFER_OFFSET_TOO_LARGE: MessageId = MessageId(243);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETINDEXBUFFER_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETINDEXBUFFER_OFFSET_UNALIGNED: MessageId = MessageId(244);
/// The header's `D3D11_MESSAGE_ID_DEVICE_VSSETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_VSSETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(245);
/// The header's `D3D11_MESSAGE_ID_VSSETCONSTANTBUFFERS_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSETCONSTANTBUFFERS_INVALIDBUFFER: MessageId = MessageId(246);
/// The header's `D3D11_MESSAGE_ID_DEVICE_VSSETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_VSSETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId = MessageId(247);
/// The header's `D3D11_MESSAGE_ID_DEVICE_VSSETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_VSSETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(248);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GSSETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GSSETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(249);
/// The header's `D3D11_MESSAGE_ID_GSSETCONSTANTBUFFERS_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GSSETCONSTANTBUFFERS_INVALIDBUFFER: MessageId = MessageId(250);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GSSETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GSSETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId = MessageId(251);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GSSETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GSSETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(252);
/// The header's `D3D11_MESSAGE_ID_SOSETTARGETS_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SOSETTARGETS_INVALIDBUFFER: MessageId = MessageId(253);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SOSETTARGETS_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SOSETTARGETS_OFFSET_UNALIGNED: MessageId = MessageId(254);
/// The header's `D3D11_MESSAGE_ID_DEVICE_PSSETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_PSSETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(255);
/// The header's `D3D11_MESSAGE_ID_PSSETCONSTANTBUFFERS_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PSSETCONSTANTBUFFERS_INVALIDBUFFER: MessageId = MessageId(0x100);
/// The header's `D3D11_MESSAGE_ID_DEVICE_PSSETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_PSSETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId = MessageId(0x101);
/// The header's `D3D11_MESSAGE_ID_DEVICE_PSSETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_PSSETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x102);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RSSETVIEWPORTS_INVALIDVIEWPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RSSETVIEWPORTS_INVALIDVIEWPORT: MessageId = MessageId(0x103);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RSSETSCISSORRECTS_INVALIDSCISSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RSSETSCISSORRECTS_INVALIDSCISSOR: MessageId = MessageId(0x104);
/// The header's `D3D11_MESSAGE_ID_CLEARRENDERTARGETVIEW_DENORMFLUSH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARRENDERTARGETVIEW_DENORMFLUSH: MessageId = MessageId(0x105);
/// The header's `D3D11_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_DENORMFLUSH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_DENORMFLUSH: MessageId = MessageId(0x106);
/// The header's `D3D11_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_INVALID: MessageId = MessageId(0x107);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IAGETVERTEXBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IAGETVERTEXBUFFERS_BUFFERS_EMPTY: MessageId = MessageId(0x108);
/// The header's `D3D11_MESSAGE_ID_DEVICE_VSGETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_VSGETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x109);
/// The header's `D3D11_MESSAGE_ID_DEVICE_VSGETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_VSGETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId = MessageId(0x10a);
/// The header's `D3D11_MESSAGE_ID_DEVICE_VSGETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_VSGETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x10b);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GSGETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GSGETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x10c);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GSGETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GSGETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId = MessageId(0x10d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GSGETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GSGETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x10e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SOGETTARGETS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SOGETTARGETS_BUFFERS_EMPTY: MessageId = MessageId(0x10f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_PSGETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_PSGETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x110);
/// The header's `D3D11_MESSAGE_ID_DEVICE_PSGETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_PSGETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId = MessageId(0x111);
/// The header's `D3D11_MESSAGE_ID_DEVICE_PSGETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_PSGETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x112);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RSGETVIEWPORTS_VIEWPORTS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RSGETVIEWPORTS_VIEWPORTS_EMPTY: MessageId = MessageId(0x113);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RSGETSCISSORRECTS_RECTS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RSGETSCISSORRECTS_RECTS_EMPTY: MessageId = MessageId(0x114);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GENERATEMIPS_RESOURCE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GENERATEMIPS_RESOURCE_INVALID: MessageId = MessageId(0x115);
/// The header's `D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDDESTINATIONSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDDESTINATIONSUBRESOURCE: MessageId =
    MessageId(0x116);
/// The header's `D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDSOURCESUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDSOURCESUBRESOURCE: MessageId =
    MessageId(0x117);
/// The header's `D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDSOURCEBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDSOURCEBOX: MessageId = MessageId(0x118);
/// The header's `D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDSOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDSOURCE: MessageId = MessageId(0x119);
/// The header's `D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDDESTINATIONSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDDESTINATIONSTATE: MessageId =
    MessageId(0x11a);
/// The header's `D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDSOURCESTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_INVALIDSOURCESTATE: MessageId = MessageId(0x11b);
/// The header's `D3D11_MESSAGE_ID_COPYRESOURCE_INVALIDSOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYRESOURCE_INVALIDSOURCE: MessageId = MessageId(0x11c);
/// The header's `D3D11_MESSAGE_ID_COPYRESOURCE_INVALIDDESTINATIONSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYRESOURCE_INVALIDDESTINATIONSTATE: MessageId = MessageId(0x11d);
/// The header's `D3D11_MESSAGE_ID_COPYRESOURCE_INVALIDSOURCESTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYRESOURCE_INVALIDSOURCESTATE: MessageId = MessageId(0x11e);
/// The header's `D3D11_MESSAGE_ID_UPDATESUBRESOURCE_INVALIDDESTINATIONSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UPDATESUBRESOURCE_INVALIDDESTINATIONSUBRESOURCE: MessageId =
    MessageId(0x11f);
/// The header's `D3D11_MESSAGE_ID_UPDATESUBRESOURCE_INVALIDDESTINATIONBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UPDATESUBRESOURCE_INVALIDDESTINATIONBOX: MessageId = MessageId(0x120);
/// The header's `D3D11_MESSAGE_ID_UPDATESUBRESOURCE_INVALIDDESTINATIONSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UPDATESUBRESOURCE_INVALIDDESTINATIONSTATE: MessageId = MessageId(0x121);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_DESTINATION_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_DESTINATION_INVALID: MessageId =
    MessageId(0x122);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_DESTINATION_SUBRESOURCE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_DESTINATION_SUBRESOURCE_INVALID: MessageId =
    MessageId(0x123);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_SOURCE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_SOURCE_INVALID: MessageId = MessageId(0x124);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_SOURCE_SUBRESOURCE_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_SOURCE_SUBRESOURCE_INVALID: MessageId =
    MessageId(0x125);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RESOLVESUBRESOURCE_FORMAT_INVALID: MessageId = MessageId(0x126);
/// The header's `D3D11_MESSAGE_ID_BUFFER_MAP_INVALIDMAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_BUFFER_MAP_INVALIDMAPTYPE: MessageId = MessageId(0x127);
/// The header's `D3D11_MESSAGE_ID_BUFFER_MAP_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_BUFFER_MAP_INVALIDFLAGS: MessageId = MessageId(0x128);
/// The header's `D3D11_MESSAGE_ID_BUFFER_MAP_ALREADYMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_BUFFER_MAP_ALREADYMAPPED: MessageId = MessageId(0x129);
/// The header's `D3D11_MESSAGE_ID_BUFFER_MAP_DEVICEREMOVED_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_BUFFER_MAP_DEVICEREMOVED_RETURN: MessageId = MessageId(0x12a);
/// The header's `D3D11_MESSAGE_ID_BUFFER_UNMAP_NOTMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_BUFFER_UNMAP_NOTMAPPED: MessageId = MessageId(0x12b);
/// The header's `D3D11_MESSAGE_ID_TEXTURE1D_MAP_INVALIDMAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE1D_MAP_INVALIDMAPTYPE: MessageId = MessageId(0x12c);
/// The header's `D3D11_MESSAGE_ID_TEXTURE1D_MAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE1D_MAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x12d);
/// The header's `D3D11_MESSAGE_ID_TEXTURE1D_MAP_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE1D_MAP_INVALIDFLAGS: MessageId = MessageId(0x12e);
/// The header's `D3D11_MESSAGE_ID_TEXTURE1D_MAP_ALREADYMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE1D_MAP_ALREADYMAPPED: MessageId = MessageId(0x12f);
/// The header's `D3D11_MESSAGE_ID_TEXTURE1D_MAP_DEVICEREMOVED_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE1D_MAP_DEVICEREMOVED_RETURN: MessageId = MessageId(0x130);
/// The header's `D3D11_MESSAGE_ID_TEXTURE1D_UNMAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE1D_UNMAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x131);
/// The header's `D3D11_MESSAGE_ID_TEXTURE1D_UNMAP_NOTMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE1D_UNMAP_NOTMAPPED: MessageId = MessageId(0x132);
/// The header's `D3D11_MESSAGE_ID_TEXTURE2D_MAP_INVALIDMAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE2D_MAP_INVALIDMAPTYPE: MessageId = MessageId(0x133);
/// The header's `D3D11_MESSAGE_ID_TEXTURE2D_MAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE2D_MAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x134);
/// The header's `D3D11_MESSAGE_ID_TEXTURE2D_MAP_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE2D_MAP_INVALIDFLAGS: MessageId = MessageId(0x135);
/// The header's `D3D11_MESSAGE_ID_TEXTURE2D_MAP_ALREADYMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE2D_MAP_ALREADYMAPPED: MessageId = MessageId(0x136);
/// The header's `D3D11_MESSAGE_ID_TEXTURE2D_MAP_DEVICEREMOVED_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE2D_MAP_DEVICEREMOVED_RETURN: MessageId = MessageId(0x137);
/// The header's `D3D11_MESSAGE_ID_TEXTURE2D_UNMAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE2D_UNMAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x138);
/// The header's `D3D11_MESSAGE_ID_TEXTURE2D_UNMAP_NOTMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE2D_UNMAP_NOTMAPPED: MessageId = MessageId(0x139);
/// The header's `D3D11_MESSAGE_ID_TEXTURE3D_MAP_INVALIDMAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE3D_MAP_INVALIDMAPTYPE: MessageId = MessageId(0x13a);
/// The header's `D3D11_MESSAGE_ID_TEXTURE3D_MAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE3D_MAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x13b);
/// The header's `D3D11_MESSAGE_ID_TEXTURE3D_MAP_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE3D_MAP_INVALIDFLAGS: MessageId = MessageId(0x13c);
/// The header's `D3D11_MESSAGE_ID_TEXTURE3D_MAP_ALREADYMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE3D_MAP_ALREADYMAPPED: MessageId = MessageId(0x13d);
/// The header's `D3D11_MESSAGE_ID_TEXTURE3D_MAP_DEVICEREMOVED_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE3D_MAP_DEVICEREMOVED_RETURN: MessageId = MessageId(0x13e);
/// The header's `D3D11_MESSAGE_ID_TEXTURE3D_UNMAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE3D_UNMAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x13f);
/// The header's `D3D11_MESSAGE_ID_TEXTURE3D_UNMAP_NOTMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXTURE3D_UNMAP_NOTMAPPED: MessageId = MessageId(0x140);
/// The header's `D3D11_MESSAGE_ID_CHECKFORMATSUPPORT_FORMAT_DEPRECATED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKFORMATSUPPORT_FORMAT_DEPRECATED: MessageId = MessageId(0x141);
/// The header's `D3D11_MESSAGE_ID_CHECKMULTISAMPLEQUALITYLEVELS_FORMAT_DEPRECATED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKMULTISAMPLEQUALITYLEVELS_FORMAT_DEPRECATED: MessageId =
    MessageId(0x142);
/// The header's `D3D11_MESSAGE_ID_SETEXCEPTIONMODE_UNRECOGNIZEDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETEXCEPTIONMODE_UNRECOGNIZEDFLAGS: MessageId = MessageId(0x143);
/// The header's `D3D11_MESSAGE_ID_SETEXCEPTIONMODE_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETEXCEPTIONMODE_INVALIDARG_RETURN: MessageId = MessageId(0x144);
/// The header's `D3D11_MESSAGE_ID_SETEXCEPTIONMODE_DEVICEREMOVED_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETEXCEPTIONMODE_DEVICEREMOVED_RETURN: MessageId = MessageId(0x145);
/// The header's `D3D11_MESSAGE_ID_REF_SIMULATING_INFINITELY_FAST_HARDWARE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_SIMULATING_INFINITELY_FAST_HARDWARE: MessageId = MessageId(0x146);
/// The header's `D3D11_MESSAGE_ID_REF_THREADING_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_THREADING_MODE: MessageId = MessageId(0x147);
/// The header's `D3D11_MESSAGE_ID_REF_UMDRIVER_EXCEPTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_UMDRIVER_EXCEPTION: MessageId = MessageId(0x148);
/// The header's `D3D11_MESSAGE_ID_REF_KMDRIVER_EXCEPTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_KMDRIVER_EXCEPTION: MessageId = MessageId(0x149);
/// The header's `D3D11_MESSAGE_ID_REF_HARDWARE_EXCEPTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_HARDWARE_EXCEPTION: MessageId = MessageId(0x14a);
/// The header's `D3D11_MESSAGE_ID_REF_ACCESSING_INDEXABLE_TEMP_OUT_OF_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_ACCESSING_INDEXABLE_TEMP_OUT_OF_RANGE: MessageId = MessageId(0x14b);
/// The header's `D3D11_MESSAGE_ID_REF_PROBLEM_PARSING_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_PROBLEM_PARSING_SHADER: MessageId = MessageId(0x14c);
/// The header's `D3D11_MESSAGE_ID_REF_OUT_OF_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_OUT_OF_MEMORY: MessageId = MessageId(0x14d);
/// The header's `D3D11_MESSAGE_ID_REF_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_INFO: MessageId = MessageId(0x14e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEXPOS_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEXPOS_OVERFLOW: MessageId = MessageId(0x14f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAWINDEXED_INDEXPOS_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAWINDEXED_INDEXPOS_OVERFLOW: MessageId = MessageId(0x150);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAWINSTANCED_VERTEXPOS_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAWINSTANCED_VERTEXPOS_OVERFLOW: MessageId = MessageId(0x151);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAWINSTANCED_INSTANCEPOS_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAWINSTANCED_INSTANCEPOS_OVERFLOW: MessageId = MessageId(0x152);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAWINDEXEDINSTANCED_INSTANCEPOS_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAWINDEXEDINSTANCED_INSTANCEPOS_OVERFLOW: MessageId =
    MessageId(0x153);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAWINDEXEDINSTANCED_INDEXPOS_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAWINDEXEDINSTANCED_INDEXPOS_OVERFLOW: MessageId =
    MessageId(0x154);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_SHADER_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_SHADER_NOT_SET: MessageId = MessageId(0x155);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_SEMANTICNAME_NOT_FOUND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_SEMANTICNAME_NOT_FOUND: MessageId =
    MessageId(0x156);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_REGISTERINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_REGISTERINDEX: MessageId = MessageId(0x157);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_COMPONENTTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_COMPONENTTYPE: MessageId = MessageId(0x158);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_REGISTERMASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_REGISTERMASK: MessageId = MessageId(0x159);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_SYSTEMVALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_SYSTEMVALUE: MessageId = MessageId(0x15a);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_NEVERWRITTEN_ALWAYSREADS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_NEVERWRITTEN_ALWAYSREADS: MessageId =
    MessageId(0x15b);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_BUFFER_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_BUFFER_NOT_SET: MessageId = MessageId(0x15c);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INPUTLAYOUT_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INPUTLAYOUT_NOT_SET: MessageId = MessageId(0x15d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_CONSTANT_BUFFER_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_CONSTANT_BUFFER_NOT_SET: MessageId = MessageId(0x15e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_CONSTANT_BUFFER_TOO_SMALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_CONSTANT_BUFFER_TOO_SMALL: MessageId = MessageId(0x15f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_SAMPLER_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_SAMPLER_NOT_SET: MessageId = MessageId(0x160);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_SHADERRESOURCEVIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_SHADERRESOURCEVIEW_NOT_SET: MessageId = MessageId(0x161);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VIEW_DIMENSION_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VIEW_DIMENSION_MISMATCH: MessageId = MessageId(0x162);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_BUFFER_STRIDE_TOO_SMALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_BUFFER_STRIDE_TOO_SMALL: MessageId = MessageId(0x163);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_BUFFER_TOO_SMALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_BUFFER_TOO_SMALL: MessageId = MessageId(0x164);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INDEX_BUFFER_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INDEX_BUFFER_NOT_SET: MessageId = MessageId(0x165);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INDEX_BUFFER_FORMAT_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INDEX_BUFFER_FORMAT_INVALID: MessageId = MessageId(0x166);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INDEX_BUFFER_TOO_SMALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INDEX_BUFFER_TOO_SMALL: MessageId = MessageId(0x167);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_GS_INPUT_PRIMITIVE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_GS_INPUT_PRIMITIVE_MISMATCH: MessageId = MessageId(0x168);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_RETURN_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_RETURN_TYPE_MISMATCH: MessageId = MessageId(0x169);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_POSITION_NOT_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_POSITION_NOT_PRESENT: MessageId = MessageId(0x16a);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_OUTPUT_STREAM_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_OUTPUT_STREAM_NOT_SET: MessageId = MessageId(0x16b);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_BOUND_RESOURCE_MAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_BOUND_RESOURCE_MAPPED: MessageId = MessageId(0x16c);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INVALID_PRIMITIVETOPOLOGY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INVALID_PRIMITIVETOPOLOGY: MessageId = MessageId(0x16d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_OFFSET_UNALIGNED: MessageId = MessageId(0x16e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_STRIDE_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VERTEX_STRIDE_UNALIGNED: MessageId = MessageId(0x16f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INDEX_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INDEX_OFFSET_UNALIGNED: MessageId = MessageId(0x170);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_OUTPUT_STREAM_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_OUTPUT_STREAM_OFFSET_UNALIGNED: MessageId = MessageId(0x171);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_FORMAT_LD_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_FORMAT_LD_UNSUPPORTED: MessageId = MessageId(0x172);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_FORMAT_SAMPLE_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_FORMAT_SAMPLE_UNSUPPORTED: MessageId =
    MessageId(0x173);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_FORMAT_SAMPLE_C_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_FORMAT_SAMPLE_C_UNSUPPORTED: MessageId =
    MessageId(0x174);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_MULTISAMPLE_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_MULTISAMPLE_UNSUPPORTED: MessageId =
    MessageId(0x175);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_SO_TARGETS_BOUND_WITHOUT_SOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_SO_TARGETS_BOUND_WITHOUT_SOURCE: MessageId =
    MessageId(0x176);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_SO_STRIDE_LARGER_THAN_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_SO_STRIDE_LARGER_THAN_BUFFER: MessageId = MessageId(0x177);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_OM_RENDER_TARGET_DOES_NOT_SUPPORT_BLENDING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_OM_RENDER_TARGET_DOES_NOT_SUPPORT_BLENDING: MessageId =
    MessageId(0x178);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_OM_DUAL_SOURCE_BLENDING_CAN_ONLY_HAVE_RENDER_TARGET_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_OM_DUAL_SOURCE_BLENDING_CAN_ONLY_HAVE_RENDER_TARGET_0:
    MessageId = MessageId(0x179);
/// The header's `D3D11_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_AT_FAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_AT_FAULT: MessageId = MessageId(0x17a);
/// The header's `D3D11_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_POSSIBLY_AT_FAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_POSSIBLY_AT_FAULT: MessageId = MessageId(0x17b);
/// The header's `D3D11_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_NOT_AT_FAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_REMOVAL_PROCESS_NOT_AT_FAULT: MessageId = MessageId(0x17c);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE_INVALIDARG_RETURN: MessageId =
    MessageId(0x17d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x17e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE_BADINTERFACE_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE_BADINTERFACE_RETURN: MessageId =
    MessageId(0x17f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_VIEWPORT_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_VIEWPORT_NOT_SET: MessageId = MessageId(0x180);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_TRAILING_DIGIT_IN_SEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_TRAILING_DIGIT_IN_SEMANTIC: MessageId =
    MessageId(0x181);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_TRAILING_DIGIT_IN_SEMANTIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_TRAILING_DIGIT_IN_SEMANTIC:
    MessageId = MessageId(0x182);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RSSETVIEWPORTS_DENORMFLUSH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RSSETVIEWPORTS_DENORMFLUSH: MessageId = MessageId(0x183);
/// The header's `D3D11_MESSAGE_ID_OMSETRENDERTARGETS_INVALIDVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OMSETRENDERTARGETS_INVALIDVIEW: MessageId = MessageId(0x184);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETTEXTFILTERSIZE_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETTEXTFILTERSIZE_INVALIDDIMENSIONS: MessageId = MessageId(0x185);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_SAMPLER_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_SAMPLER_MISMATCH: MessageId = MessageId(0x186);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_TYPE_MISMATCH: MessageId = MessageId(0x187);
/// The header's `D3D11_MESSAGE_ID_BLENDSTATE_GETDESC_LEGACY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_BLENDSTATE_GETDESC_LEGACY: MessageId = MessageId(0x188);
/// The header's `D3D11_MESSAGE_ID_SHADERRESOURCEVIEW_GETDESC_LEGACY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SHADERRESOURCEVIEW_GETDESC_LEGACY: MessageId = MessageId(0x189);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERY_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERY_OUTOFMEMORY_RETURN: MessageId = MessageId(0x18a);
/// The header's `D3D11_MESSAGE_ID_CREATEPREDICATE_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEPREDICATE_OUTOFMEMORY_RETURN: MessageId = MessageId(0x18b);
/// The header's `D3D11_MESSAGE_ID_CREATECOUNTER_OUTOFRANGE_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOUNTER_OUTOFRANGE_COUNTER: MessageId = MessageId(0x18c);
/// The header's `D3D11_MESSAGE_ID_CREATECOUNTER_SIMULTANEOUS_ACTIVE_COUNTERS_EXHAUSTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOUNTER_SIMULTANEOUS_ACTIVE_COUNTERS_EXHAUSTED: MessageId =
    MessageId(0x18d);
/// The header's `D3D11_MESSAGE_ID_CREATECOUNTER_UNSUPPORTED_WELLKNOWN_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOUNTER_UNSUPPORTED_WELLKNOWN_COUNTER: MessageId =
    MessageId(0x18e);
/// The header's `D3D11_MESSAGE_ID_CREATECOUNTER_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOUNTER_OUTOFMEMORY_RETURN: MessageId = MessageId(0x18f);
/// The header's `D3D11_MESSAGE_ID_CREATECOUNTER_NONEXCLUSIVE_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOUNTER_NONEXCLUSIVE_RETURN: MessageId = MessageId(0x190);
/// The header's `D3D11_MESSAGE_ID_CREATECOUNTER_NULLDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOUNTER_NULLDESC: MessageId = MessageId(0x191);
/// The header's `D3D11_MESSAGE_ID_CHECKCOUNTER_OUTOFRANGE_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKCOUNTER_OUTOFRANGE_COUNTER: MessageId = MessageId(0x192);
/// The header's `D3D11_MESSAGE_ID_CHECKCOUNTER_UNSUPPORTED_WELLKNOWN_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKCOUNTER_UNSUPPORTED_WELLKNOWN_COUNTER: MessageId = MessageId(0x193);
/// The header's `D3D11_MESSAGE_ID_SETPREDICATION_INVALID_PREDICATE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETPREDICATION_INVALID_PREDICATE_STATE: MessageId = MessageId(0x194);
/// The header's `D3D11_MESSAGE_ID_QUERY_BEGIN_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERY_BEGIN_UNSUPPORTED: MessageId = MessageId(0x195);
/// The header's `D3D11_MESSAGE_ID_PREDICATE_BEGIN_DURING_PREDICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PREDICATE_BEGIN_DURING_PREDICATION: MessageId = MessageId(0x196);
/// The header's `D3D11_MESSAGE_ID_QUERY_BEGIN_DUPLICATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERY_BEGIN_DUPLICATE: MessageId = MessageId(0x197);
/// The header's `D3D11_MESSAGE_ID_QUERY_BEGIN_ABANDONING_PREVIOUS_RESULTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERY_BEGIN_ABANDONING_PREVIOUS_RESULTS: MessageId = MessageId(0x198);
/// The header's `D3D11_MESSAGE_ID_PREDICATE_END_DURING_PREDICATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PREDICATE_END_DURING_PREDICATION: MessageId = MessageId(0x199);
/// The header's `D3D11_MESSAGE_ID_QUERY_END_ABANDONING_PREVIOUS_RESULTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERY_END_ABANDONING_PREVIOUS_RESULTS: MessageId = MessageId(0x19a);
/// The header's `D3D11_MESSAGE_ID_QUERY_END_WITHOUT_BEGIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERY_END_WITHOUT_BEGIN: MessageId = MessageId(0x19b);
/// The header's `D3D11_MESSAGE_ID_QUERY_GETDATA_INVALID_DATASIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERY_GETDATA_INVALID_DATASIZE: MessageId = MessageId(0x19c);
/// The header's `D3D11_MESSAGE_ID_QUERY_GETDATA_INVALID_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERY_GETDATA_INVALID_FLAGS: MessageId = MessageId(0x19d);
/// The header's `D3D11_MESSAGE_ID_QUERY_GETDATA_INVALID_CALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERY_GETDATA_INVALID_CALL: MessageId = MessageId(0x19e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_PS_OUTPUT_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_PS_OUTPUT_TYPE_MISMATCH: MessageId = MessageId(0x19f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_FORMAT_GATHER_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_FORMAT_GATHER_UNSUPPORTED: MessageId =
    MessageId(0x1a0);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INVALID_USE_OF_CENTER_MULTISAMPLE_PATTERN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INVALID_USE_OF_CENTER_MULTISAMPLE_PATTERN: MessageId =
    MessageId(0x1a1);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_STRIDE_TOO_LARGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_STRIDE_TOO_LARGE: MessageId = MessageId(0x1a2);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_INVALIDRANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETVERTEXBUFFERS_INVALIDRANGE: MessageId = MessageId(0x1a3);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_EMPTY_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_EMPTY_LAYOUT: MessageId = MessageId(0x1a4);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_SAMPLE_COUNT_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RESOURCE_SAMPLE_COUNT_MISMATCH: MessageId = MessageId(0x1a5);
/// The header's `D3D11_MESSAGE_ID_LIVE_OBJECT_SUMMARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_OBJECT_SUMMARY: MessageId = MessageId(0x1a6);
/// The header's `D3D11_MESSAGE_ID_LIVE_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_BUFFER: MessageId = MessageId(0x1a7);
/// The header's `D3D11_MESSAGE_ID_LIVE_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_TEXTURE1D: MessageId = MessageId(0x1a8);
/// The header's `D3D11_MESSAGE_ID_LIVE_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_TEXTURE2D: MessageId = MessageId(0x1a9);
/// The header's `D3D11_MESSAGE_ID_LIVE_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_TEXTURE3D: MessageId = MessageId(0x1aa);
/// The header's `D3D11_MESSAGE_ID_LIVE_SHADERRESOURCEVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_SHADERRESOURCEVIEW: MessageId = MessageId(0x1ab);
/// The header's `D3D11_MESSAGE_ID_LIVE_RENDERTARGETVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_RENDERTARGETVIEW: MessageId = MessageId(0x1ac);
/// The header's `D3D11_MESSAGE_ID_LIVE_DEPTHSTENCILVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DEPTHSTENCILVIEW: MessageId = MessageId(0x1ad);
/// The header's `D3D11_MESSAGE_ID_LIVE_VERTEXSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_VERTEXSHADER: MessageId = MessageId(0x1ae);
/// The header's `D3D11_MESSAGE_ID_LIVE_GEOMETRYSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_GEOMETRYSHADER: MessageId = MessageId(0x1af);
/// The header's `D3D11_MESSAGE_ID_LIVE_PIXELSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_PIXELSHADER: MessageId = MessageId(0x1b0);
/// The header's `D3D11_MESSAGE_ID_LIVE_INPUTLAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_INPUTLAYOUT: MessageId = MessageId(0x1b1);
/// The header's `D3D11_MESSAGE_ID_LIVE_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_SAMPLER: MessageId = MessageId(0x1b2);
/// The header's `D3D11_MESSAGE_ID_LIVE_BLENDSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_BLENDSTATE: MessageId = MessageId(0x1b3);
/// The header's `D3D11_MESSAGE_ID_LIVE_DEPTHSTENCILSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DEPTHSTENCILSTATE: MessageId = MessageId(0x1b4);
/// The header's `D3D11_MESSAGE_ID_LIVE_RASTERIZERSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_RASTERIZERSTATE: MessageId = MessageId(0x1b5);
/// The header's `D3D11_MESSAGE_ID_LIVE_QUERY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_QUERY: MessageId = MessageId(0x1b6);
/// The header's `D3D11_MESSAGE_ID_LIVE_PREDICATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_PREDICATE: MessageId = MessageId(0x1b7);
/// The header's `D3D11_MESSAGE_ID_LIVE_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_COUNTER: MessageId = MessageId(0x1b8);
/// The header's `D3D11_MESSAGE_ID_LIVE_DEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DEVICE: MessageId = MessageId(0x1b9);
/// The header's `D3D11_MESSAGE_ID_LIVE_SWAPCHAIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_SWAPCHAIN: MessageId = MessageId(0x1ba);
/// The header's `D3D11_MESSAGE_ID_D3D10_MESSAGES_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D10_MESSAGES_END: MessageId = MessageId(0x1bb);
/// The header's `D3D11_MESSAGE_ID_D3D10L9_MESSAGES_START`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D10L9_MESSAGES_START: MessageId = MessageId(0x100000);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_STENCIL_NO_TWO_SIDED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILSTATE_STENCIL_NO_TWO_SIDED: MessageId =
    MessageId(0x100001);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_DepthBiasClamp_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_DEPTHBIASCLAMP_NOT_SUPPORTED: MessageId =
    MessageId(0x100002);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_NO_COMPARISON_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_NO_COMPARISON_SUPPORT: MessageId =
    MessageId(0x100003);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_EXCESSIVE_ANISOTROPY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_EXCESSIVE_ANISOTROPY: MessageId = MessageId(0x100004);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_BORDER_OUT_OF_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_BORDER_OUT_OF_RANGE: MessageId = MessageId(0x100005);
/// The header's `D3D11_MESSAGE_ID_VSSETSAMPLERS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSETSAMPLERS_NOT_SUPPORTED: MessageId = MessageId(0x100006);
/// The header's `D3D11_MESSAGE_ID_VSSETSAMPLERS_TOO_MANY_SAMPLERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSETSAMPLERS_TOO_MANY_SAMPLERS: MessageId = MessageId(0x100007);
/// The header's `D3D11_MESSAGE_ID_PSSETSAMPLERS_TOO_MANY_SAMPLERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PSSETSAMPLERS_TOO_MANY_SAMPLERS: MessageId = MessageId(0x100008);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NO_ARRAYS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NO_ARRAYS: MessageId = MessageId(0x100009);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NO_VB_AND_IB_BIND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NO_VB_AND_IB_BIND: MessageId = MessageId(0x10000a);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NO_TEXTURE_1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NO_TEXTURE_1D: MessageId = MessageId(0x10000b);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_DIMENSION_OUT_OF_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_DIMENSION_OUT_OF_RANGE: MessageId = MessageId(0x10000c);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NOT_BINDABLE_AS_SHADER_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NOT_BINDABLE_AS_SHADER_RESOURCE: MessageId =
    MessageId(0x10000d);
/// The header's `D3D11_MESSAGE_ID_OMSETRENDERTARGETS_TOO_MANY_RENDER_TARGETS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OMSETRENDERTARGETS_TOO_MANY_RENDER_TARGETS: MessageId =
    MessageId(0x10000e);
/// The header's `D3D11_MESSAGE_ID_OMSETRENDERTARGETS_NO_DIFFERING_BIT_DEPTHS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OMSETRENDERTARGETS_NO_DIFFERING_BIT_DEPTHS: MessageId =
    MessageId(0x10000f);
/// The header's `D3D11_MESSAGE_ID_IASETVERTEXBUFFERS_BAD_BUFFER_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_IASETVERTEXBUFFERS_BAD_BUFFER_INDEX: MessageId = MessageId(0x100010);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RSSETVIEWPORTS_TOO_MANY_VIEWPORTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RSSETVIEWPORTS_TOO_MANY_VIEWPORTS: MessageId =
    MessageId(0x100011);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETPRIMITIVETOPOLOGY_ADJACENCY_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETPRIMITIVETOPOLOGY_ADJACENCY_UNSUPPORTED: MessageId =
    MessageId(0x100012);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RSSETSCISSORRECTS_TOO_MANY_SCISSORS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RSSETSCISSORRECTS_TOO_MANY_SCISSORS: MessageId =
    MessageId(0x100013);
/// The header's `D3D11_MESSAGE_ID_COPYRESOURCE_ONLY_TEXTURE_2D_WITHIN_GPU_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYRESOURCE_ONLY_TEXTURE_2D_WITHIN_GPU_MEMORY: MessageId =
    MessageId(0x100014);
/// The header's `D3D11_MESSAGE_ID_COPYRESOURCE_NO_TEXTURE_3D_READBACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYRESOURCE_NO_TEXTURE_3D_READBACK: MessageId = MessageId(0x100015);
/// The header's `D3D11_MESSAGE_ID_COPYRESOURCE_NO_TEXTURE_ONLY_READBACK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYRESOURCE_NO_TEXTURE_ONLY_READBACK: MessageId = MessageId(0x100016);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_UNSUPPORTED_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_UNSUPPORTED_FORMAT: MessageId = MessageId(0x100017);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_ALPHA_TO_COVERAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_ALPHA_TO_COVERAGE: MessageId = MessageId(0x100018);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_DepthClipEnable_MUST_BE_TRUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_DEPTHCLIPENABLE_MUST_BE_TRUE: MessageId =
    MessageId(0x100019);
/// The header's `D3D11_MESSAGE_ID_DRAWINDEXED_STARTINDEXLOCATION_MUST_BE_POSITIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DRAWINDEXED_STARTINDEXLOCATION_MUST_BE_POSITIVE: MessageId =
    MessageId(0x10001a);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_MUST_USE_LOWEST_LOD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_MUST_USE_LOWEST_LOD: MessageId =
    MessageId(0x10001b);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_MINLOD_MUST_NOT_BE_FRACTIONAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_MINLOD_MUST_NOT_BE_FRACTIONAL: MessageId =
    MessageId(0x10001c);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_MAXLOD_MUST_BE_FLT_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_MAXLOD_MUST_BE_FLT_MAX: MessageId =
    MessageId(0x10001d);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_FIRSTARRAYSLICE_MUST_BE_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_FIRSTARRAYSLICE_MUST_BE_ZERO: MessageId =
    MessageId(0x10001e);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_CUBES_MUST_HAVE_6_SIDES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_CUBES_MUST_HAVE_6_SIDES: MessageId =
    MessageId(0x10001f);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NOT_BINDABLE_AS_RENDER_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NOT_BINDABLE_AS_RENDER_TARGET: MessageId =
    MessageId(0x100020);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NO_DWORD_INDEX_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NO_DWORD_INDEX_BUFFER: MessageId = MessageId(0x100021);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_MSAA_PRECLUDES_SHADER_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_MSAA_PRECLUDES_SHADER_RESOURCE: MessageId =
    MessageId(0x100022);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_PRESENTATION_PRECLUDES_SHADER_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_PRESENTATION_PRECLUDES_SHADER_RESOURCE: MessageId =
    MessageId(0x100023);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_INDEPENDENT_BLEND_ENABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_INDEPENDENT_BLEND_ENABLE: MessageId =
    MessageId(0x100024);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_INDEPENDENT_WRITE_MASKS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_INDEPENDENT_WRITE_MASKS: MessageId =
    MessageId(0x100025);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NO_STREAM_OUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NO_STREAM_OUT: MessageId = MessageId(0x100026);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_ONLY_VB_IB_FOR_BUFFERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_ONLY_VB_IB_FOR_BUFFERS: MessageId = MessageId(0x100027);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NO_AUTOGEN_FOR_VOLUMES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NO_AUTOGEN_FOR_VOLUMES: MessageId = MessageId(0x100028);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_DXGI_FORMAT_R8G8B8A8_CANNOT_BE_SHARED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_DXGI_FORMAT_R8G8B8A8_CANNOT_BE_SHARED: MessageId =
    MessageId(0x100029);
/// The header's `D3D11_MESSAGE_ID_VSSHADERRESOURCES_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSHADERRESOURCES_NOT_SUPPORTED: MessageId = MessageId(0x10002a);
/// The header's `D3D11_MESSAGE_ID_GEOMETRY_SHADER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GEOMETRY_SHADER_NOT_SUPPORTED: MessageId = MessageId(0x10002b);
/// The header's `D3D11_MESSAGE_ID_STREAM_OUT_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_STREAM_OUT_NOT_SUPPORTED: MessageId = MessageId(0x10002c);
/// The header's `D3D11_MESSAGE_ID_TEXT_FILTER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TEXT_FILTER_NOT_SUPPORTED: MessageId = MessageId(0x10002d);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_SEPARATE_ALPHA_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_SEPARATE_ALPHA_BLEND: MessageId =
    MessageId(0x10002e);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_MRT_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_NO_MRT_BLEND: MessageId = MessageId(0x10002f);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_OPERATION_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_OPERATION_NOT_SUPPORTED: MessageId =
    MessageId(0x100030);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_NO_MIRRORONCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_NO_MIRRORONCE: MessageId = MessageId(0x100031);
/// The header's `D3D11_MESSAGE_ID_DRAWINSTANCED_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DRAWINSTANCED_NOT_SUPPORTED: MessageId = MessageId(0x100032);
/// The header's `D3D11_MESSAGE_ID_DRAWINDEXEDINSTANCED_NOT_SUPPORTED_BELOW_9_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DRAWINDEXEDINSTANCED_NOT_SUPPORTED_BELOW_9_3: MessageId =
    MessageId(0x100033);
/// The header's `D3D11_MESSAGE_ID_DRAWINDEXED_POINTLIST_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DRAWINDEXED_POINTLIST_UNSUPPORTED: MessageId = MessageId(0x100034);
/// The header's `D3D11_MESSAGE_ID_SETBLENDSTATE_SAMPLE_MASK_CANNOT_BE_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SETBLENDSTATE_SAMPLE_MASK_CANNOT_BE_ZERO: MessageId =
    MessageId(0x100035);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_DIMENSION_EXCEEDS_FEATURE_LEVEL_DEFINITION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_DIMENSION_EXCEEDS_FEATURE_LEVEL_DEFINITION: MessageId =
    MessageId(0x100036);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_ONLY_SINGLE_MIP_LEVEL_DEPTH_STENCIL_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_ONLY_SINGLE_MIP_LEVEL_DEPTH_STENCIL_SUPPORTED: MessageId =
    MessageId(0x100037);
/// The header's `D3D11_MESSAGE_ID_DEVICE_RSSETSCISSORRECTS_NEGATIVESCISSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_RSSETSCISSORRECTS_NEGATIVESCISSOR: MessageId =
    MessageId(0x100038);
/// The header's `D3D11_MESSAGE_ID_SLOT_ZERO_MUST_BE_D3D10_INPUT_PER_VERTEX_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SLOT_ZERO_MUST_BE_D3D10_INPUT_PER_VERTEX_DATA: MessageId =
    MessageId(0x100039);
/// The header's `D3D11_MESSAGE_ID_CREATERESOURCE_NON_POW_2_MIPMAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERESOURCE_NON_POW_2_MIPMAP: MessageId = MessageId(0x10003a);
/// The header's `D3D11_MESSAGE_ID_CREATESAMPLERSTATE_BORDER_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESAMPLERSTATE_BORDER_NOT_SUPPORTED: MessageId = MessageId(0x10003b);
/// The header's `D3D11_MESSAGE_ID_OMSETRENDERTARGETS_NO_SRGB_MRT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OMSETRENDERTARGETS_NO_SRGB_MRT: MessageId = MessageId(0x10003c);
/// The header's `D3D11_MESSAGE_ID_COPYRESOURCE_NO_3D_MISMATCHED_UPDATES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYRESOURCE_NO_3D_MISMATCHED_UPDATES: MessageId = MessageId(0x10003d);
/// The header's `D3D11_MESSAGE_ID_D3D10L9_MESSAGES_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D10L9_MESSAGES_END: MessageId = MessageId(0x10003e);
/// The header's `D3D11_MESSAGE_ID_D3D11_MESSAGES_START`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_MESSAGES_START: MessageId = MessageId(0x200000);
/// The header's `D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEPTHSTENCILVIEW_INVALIDFLAGS: MessageId = MessageId(0x200001);
/// The header's `D3D11_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVERTEXSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x200002);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADER_INVALIDCLASSLINKAGE: MessageId =
    MessageId(0x200003);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMSTREAMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMSTREAMS: MessageId =
    MessageId(0x200004);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTREAMTORASTERIZER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTREAMTORASTERIZER:
    MessageId = MessageId(0x200005);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDSTREAMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDSTREAMS: MessageId =
    MessageId(0x200006);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDCLASSLINKAGE: MessageId =
    MessageId(0x200007);
/// The header's `D3D11_MESSAGE_ID_CREATEPIXELSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEPIXELSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x200008);
/// The header's `D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_INVALID_COMMANDLISTFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_INVALID_COMMANDLISTFLAGS: MessageId =
    MessageId(0x200009);
/// The header's `D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_SINGLETHREADED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_SINGLETHREADED: MessageId = MessageId(0x20000a);
/// The header's `D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_INVALIDARG_RETURN: MessageId = MessageId(0x20000b);
/// The header's `D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_INVALID_CALL_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_INVALID_CALL_RETURN: MessageId =
    MessageId(0x20000c);
/// The header's `D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEFERREDCONTEXT_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x20000d);
/// The header's `D3D11_MESSAGE_ID_FINISHDISPLAYLIST_ONIMMEDIATECONTEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_FINISHDISPLAYLIST_ONIMMEDIATECONTEXT: MessageId = MessageId(0x20000e);
/// The header's `D3D11_MESSAGE_ID_FINISHDISPLAYLIST_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_FINISHDISPLAYLIST_OUTOFMEMORY_RETURN: MessageId = MessageId(0x20000f);
/// The header's `D3D11_MESSAGE_ID_FINISHDISPLAYLIST_INVALID_CALL_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_FINISHDISPLAYLIST_INVALID_CALL_RETURN: MessageId = MessageId(0x200010);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDSTREAM: MessageId =
    MessageId(0x200011);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDENTRIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDENTRIES: MessageId =
    MessageId(0x200012);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDSTRIDES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UNEXPECTEDSTRIDES: MessageId =
    MessageId(0x200013);
/// The header's `D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMSTRIDES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_INVALIDNUMSTRIDES: MessageId =
    MessageId(0x200014);
/// The header's `D3D11_MESSAGE_ID_DEVICE_HSSETSHADERRESOURCES_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_HSSETSHADERRESOURCES_HAZARD: MessageId = MessageId(0x200015);
/// The header's `D3D11_MESSAGE_ID_DEVICE_HSSETCONSTANTBUFFERS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_HSSETCONSTANTBUFFERS_HAZARD: MessageId = MessageId(0x200016);
/// The header's `D3D11_MESSAGE_ID_HSSETSHADERRESOURCES_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_HSSETSHADERRESOURCES_UNBINDDELETINGOBJECT: MessageId =
    MessageId(0x200017);
/// The header's `D3D11_MESSAGE_ID_HSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_HSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT: MessageId =
    MessageId(0x200018);
/// The header's `D3D11_MESSAGE_ID_CREATEHULLSHADER_INVALIDCALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEHULLSHADER_INVALIDCALL: MessageId = MessageId(0x200019);
/// The header's `D3D11_MESSAGE_ID_CREATEHULLSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEHULLSHADER_OUTOFMEMORY: MessageId = MessageId(0x20001a);
/// The header's `D3D11_MESSAGE_ID_CREATEHULLSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEHULLSHADER_INVALIDSHADERBYTECODE: MessageId = MessageId(0x20001b);
/// The header's `D3D11_MESSAGE_ID_CREATEHULLSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEHULLSHADER_INVALIDSHADERTYPE: MessageId = MessageId(0x20001c);
/// The header's `D3D11_MESSAGE_ID_CREATEHULLSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEHULLSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x20001d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_HSSETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_HSSETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x20001e);
/// The header's `D3D11_MESSAGE_ID_HSSETCONSTANTBUFFERS_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_HSSETCONSTANTBUFFERS_INVALIDBUFFER: MessageId = MessageId(0x20001f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_HSSETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_HSSETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId =
    MessageId(0x200020);
/// The header's `D3D11_MESSAGE_ID_DEVICE_HSSETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_HSSETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x200021);
/// The header's `D3D11_MESSAGE_ID_DEVICE_HSGETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_HSGETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x200022);
/// The header's `D3D11_MESSAGE_ID_DEVICE_HSGETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_HSGETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId =
    MessageId(0x200023);
/// The header's `D3D11_MESSAGE_ID_DEVICE_HSGETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_HSGETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x200024);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DSSETSHADERRESOURCES_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DSSETSHADERRESOURCES_HAZARD: MessageId = MessageId(0x200025);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DSSETCONSTANTBUFFERS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DSSETCONSTANTBUFFERS_HAZARD: MessageId = MessageId(0x200026);
/// The header's `D3D11_MESSAGE_ID_DSSETSHADERRESOURCES_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DSSETSHADERRESOURCES_UNBINDDELETINGOBJECT: MessageId =
    MessageId(0x200027);
/// The header's `D3D11_MESSAGE_ID_DSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT: MessageId =
    MessageId(0x200028);
/// The header's `D3D11_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDCALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDCALL: MessageId = MessageId(0x200029);
/// The header's `D3D11_MESSAGE_ID_CREATEDOMAINSHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDOMAINSHADER_OUTOFMEMORY: MessageId = MessageId(0x20002a);
/// The header's `D3D11_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDSHADERBYTECODE: MessageId =
    MessageId(0x20002b);
/// The header's `D3D11_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDSHADERTYPE: MessageId = MessageId(0x20002c);
/// The header's `D3D11_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDOMAINSHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x20002d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DSSETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DSSETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x20002e);
/// The header's `D3D11_MESSAGE_ID_DSSETCONSTANTBUFFERS_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DSSETCONSTANTBUFFERS_INVALIDBUFFER: MessageId = MessageId(0x20002f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DSSETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DSSETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId =
    MessageId(0x200030);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DSSETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DSSETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x200031);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DSGETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DSGETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x200032);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DSGETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DSGETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId =
    MessageId(0x200033);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DSGETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DSGETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x200034);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_HS_XOR_DS_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_HS_XOR_DS_MISMATCH: MessageId = MessageId(0x200035);
/// The header's `D3D11_MESSAGE_ID_DEFERRED_CONTEXT_REMOVAL_PROCESS_AT_FAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEFERRED_CONTEXT_REMOVAL_PROCESS_AT_FAULT: MessageId =
    MessageId(0x200036);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAWINDIRECT_INVALID_ARG_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAWINDIRECT_INVALID_ARG_BUFFER: MessageId = MessageId(0x200037);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAWINDIRECT_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAWINDIRECT_OFFSET_UNALIGNED: MessageId = MessageId(0x200038);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAWINDIRECT_OFFSET_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAWINDIRECT_OFFSET_OVERFLOW: MessageId = MessageId(0x200039);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_MAP_INVALIDMAPTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_MAP_INVALIDMAPTYPE: MessageId = MessageId(0x20003a);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_MAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_MAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x20003b);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_MAP_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_MAP_INVALIDFLAGS: MessageId = MessageId(0x20003c);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_MAP_ALREADYMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_MAP_ALREADYMAPPED: MessageId = MessageId(0x20003d);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_MAP_DEVICEREMOVED_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_MAP_DEVICEREMOVED_RETURN: MessageId = MessageId(0x20003e);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_MAP_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_MAP_OUTOFMEMORY_RETURN: MessageId = MessageId(0x20003f);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_MAP_WITHOUT_INITIAL_DISCARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_MAP_WITHOUT_INITIAL_DISCARD: MessageId = MessageId(0x200040);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_UNMAP_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_UNMAP_INVALIDSUBRESOURCE: MessageId = MessageId(0x200041);
/// The header's `D3D11_MESSAGE_ID_RESOURCE_UNMAP_NOTMAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESOURCE_UNMAP_NOTMAPPED: MessageId = MessageId(0x200042);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RASTERIZING_CONTROL_POINTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RASTERIZING_CONTROL_POINTS: MessageId = MessageId(0x200043);
/// The header's `D3D11_MESSAGE_ID_DEVICE_IASETPRIMITIVETOPOLOGY_TOPOLOGY_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_IASETPRIMITIVETOPOLOGY_TOPOLOGY_UNSUPPORTED: MessageId =
    MessageId(0x200044);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_HS_DS_SIGNATURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_HS_DS_SIGNATURE_MISMATCH: MessageId = MessageId(0x200045);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_HULL_SHADER_INPUT_TOPOLOGY_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_HULL_SHADER_INPUT_TOPOLOGY_MISMATCH: MessageId =
    MessageId(0x200046);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_HS_DS_CONTROL_POINT_COUNT_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_HS_DS_CONTROL_POINT_COUNT_MISMATCH: MessageId =
    MessageId(0x200047);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_HS_DS_TESSELLATOR_DOMAIN_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_HS_DS_TESSELLATOR_DOMAIN_MISMATCH: MessageId =
    MessageId(0x200048);
/// The header's `D3D11_MESSAGE_ID_CREATE_CONTEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_CONTEXT: MessageId = MessageId(0x200049);
/// The header's `D3D11_MESSAGE_ID_LIVE_CONTEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_CONTEXT: MessageId = MessageId(0x20004a);
/// The header's `D3D11_MESSAGE_ID_DESTROY_CONTEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_CONTEXT: MessageId = MessageId(0x20004b);
/// The header's `D3D11_MESSAGE_ID_CREATE_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_BUFFER: MessageId = MessageId(0x20004c);
/// The header's `D3D11_MESSAGE_ID_LIVE_BUFFER_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_BUFFER_WIN7: MessageId = MessageId(0x20004d);
/// The header's `D3D11_MESSAGE_ID_DESTROY_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_BUFFER: MessageId = MessageId(0x20004e);
/// The header's `D3D11_MESSAGE_ID_CREATE_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_TEXTURE1D: MessageId = MessageId(0x20004f);
/// The header's `D3D11_MESSAGE_ID_LIVE_TEXTURE1D_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_TEXTURE1D_WIN7: MessageId = MessageId(0x200050);
/// The header's `D3D11_MESSAGE_ID_DESTROY_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_TEXTURE1D: MessageId = MessageId(0x200051);
/// The header's `D3D11_MESSAGE_ID_CREATE_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_TEXTURE2D: MessageId = MessageId(0x200052);
/// The header's `D3D11_MESSAGE_ID_LIVE_TEXTURE2D_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_TEXTURE2D_WIN7: MessageId = MessageId(0x200053);
/// The header's `D3D11_MESSAGE_ID_DESTROY_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_TEXTURE2D: MessageId = MessageId(0x200054);
/// The header's `D3D11_MESSAGE_ID_CREATE_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_TEXTURE3D: MessageId = MessageId(0x200055);
/// The header's `D3D11_MESSAGE_ID_LIVE_TEXTURE3D_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_TEXTURE3D_WIN7: MessageId = MessageId(0x200056);
/// The header's `D3D11_MESSAGE_ID_DESTROY_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_TEXTURE3D: MessageId = MessageId(0x200057);
/// The header's `D3D11_MESSAGE_ID_CREATE_SHADERRESOURCEVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_SHADERRESOURCEVIEW: MessageId = MessageId(0x200058);
/// The header's `D3D11_MESSAGE_ID_LIVE_SHADERRESOURCEVIEW_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_SHADERRESOURCEVIEW_WIN7: MessageId = MessageId(0x200059);
/// The header's `D3D11_MESSAGE_ID_DESTROY_SHADERRESOURCEVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_SHADERRESOURCEVIEW: MessageId = MessageId(0x20005a);
/// The header's `D3D11_MESSAGE_ID_CREATE_RENDERTARGETVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_RENDERTARGETVIEW: MessageId = MessageId(0x20005b);
/// The header's `D3D11_MESSAGE_ID_LIVE_RENDERTARGETVIEW_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_RENDERTARGETVIEW_WIN7: MessageId = MessageId(0x20005c);
/// The header's `D3D11_MESSAGE_ID_DESTROY_RENDERTARGETVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_RENDERTARGETVIEW: MessageId = MessageId(0x20005d);
/// The header's `D3D11_MESSAGE_ID_CREATE_DEPTHSTENCILVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_DEPTHSTENCILVIEW: MessageId = MessageId(0x20005e);
/// The header's `D3D11_MESSAGE_ID_LIVE_DEPTHSTENCILVIEW_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DEPTHSTENCILVIEW_WIN7: MessageId = MessageId(0x20005f);
/// The header's `D3D11_MESSAGE_ID_DESTROY_DEPTHSTENCILVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_DEPTHSTENCILVIEW: MessageId = MessageId(0x200060);
/// The header's `D3D11_MESSAGE_ID_CREATE_VERTEXSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_VERTEXSHADER: MessageId = MessageId(0x200061);
/// The header's `D3D11_MESSAGE_ID_LIVE_VERTEXSHADER_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_VERTEXSHADER_WIN7: MessageId = MessageId(0x200062);
/// The header's `D3D11_MESSAGE_ID_DESTROY_VERTEXSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_VERTEXSHADER: MessageId = MessageId(0x200063);
/// The header's `D3D11_MESSAGE_ID_CREATE_HULLSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_HULLSHADER: MessageId = MessageId(0x200064);
/// The header's `D3D11_MESSAGE_ID_LIVE_HULLSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_HULLSHADER: MessageId = MessageId(0x200065);
/// The header's `D3D11_MESSAGE_ID_DESTROY_HULLSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_HULLSHADER: MessageId = MessageId(0x200066);
/// The header's `D3D11_MESSAGE_ID_CREATE_DOMAINSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_DOMAINSHADER: MessageId = MessageId(0x200067);
/// The header's `D3D11_MESSAGE_ID_LIVE_DOMAINSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DOMAINSHADER: MessageId = MessageId(0x200068);
/// The header's `D3D11_MESSAGE_ID_DESTROY_DOMAINSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_DOMAINSHADER: MessageId = MessageId(0x200069);
/// The header's `D3D11_MESSAGE_ID_CREATE_GEOMETRYSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_GEOMETRYSHADER: MessageId = MessageId(0x20006a);
/// The header's `D3D11_MESSAGE_ID_LIVE_GEOMETRYSHADER_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_GEOMETRYSHADER_WIN7: MessageId = MessageId(0x20006b);
/// The header's `D3D11_MESSAGE_ID_DESTROY_GEOMETRYSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_GEOMETRYSHADER: MessageId = MessageId(0x20006c);
/// The header's `D3D11_MESSAGE_ID_CREATE_PIXELSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_PIXELSHADER: MessageId = MessageId(0x20006d);
/// The header's `D3D11_MESSAGE_ID_LIVE_PIXELSHADER_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_PIXELSHADER_WIN7: MessageId = MessageId(0x20006e);
/// The header's `D3D11_MESSAGE_ID_DESTROY_PIXELSHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_PIXELSHADER: MessageId = MessageId(0x20006f);
/// The header's `D3D11_MESSAGE_ID_CREATE_INPUTLAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_INPUTLAYOUT: MessageId = MessageId(0x200070);
/// The header's `D3D11_MESSAGE_ID_LIVE_INPUTLAYOUT_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_INPUTLAYOUT_WIN7: MessageId = MessageId(0x200071);
/// The header's `D3D11_MESSAGE_ID_DESTROY_INPUTLAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_INPUTLAYOUT: MessageId = MessageId(0x200072);
/// The header's `D3D11_MESSAGE_ID_CREATE_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_SAMPLER: MessageId = MessageId(0x200073);
/// The header's `D3D11_MESSAGE_ID_LIVE_SAMPLER_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_SAMPLER_WIN7: MessageId = MessageId(0x200074);
/// The header's `D3D11_MESSAGE_ID_DESTROY_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_SAMPLER: MessageId = MessageId(0x200075);
/// The header's `D3D11_MESSAGE_ID_CREATE_BLENDSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_BLENDSTATE: MessageId = MessageId(0x200076);
/// The header's `D3D11_MESSAGE_ID_LIVE_BLENDSTATE_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_BLENDSTATE_WIN7: MessageId = MessageId(0x200077);
/// The header's `D3D11_MESSAGE_ID_DESTROY_BLENDSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_BLENDSTATE: MessageId = MessageId(0x200078);
/// The header's `D3D11_MESSAGE_ID_CREATE_DEPTHSTENCILSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_DEPTHSTENCILSTATE: MessageId = MessageId(0x200079);
/// The header's `D3D11_MESSAGE_ID_LIVE_DEPTHSTENCILSTATE_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DEPTHSTENCILSTATE_WIN7: MessageId = MessageId(0x20007a);
/// The header's `D3D11_MESSAGE_ID_DESTROY_DEPTHSTENCILSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_DEPTHSTENCILSTATE: MessageId = MessageId(0x20007b);
/// The header's `D3D11_MESSAGE_ID_CREATE_RASTERIZERSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_RASTERIZERSTATE: MessageId = MessageId(0x20007c);
/// The header's `D3D11_MESSAGE_ID_LIVE_RASTERIZERSTATE_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_RASTERIZERSTATE_WIN7: MessageId = MessageId(0x20007d);
/// The header's `D3D11_MESSAGE_ID_DESTROY_RASTERIZERSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_RASTERIZERSTATE: MessageId = MessageId(0x20007e);
/// The header's `D3D11_MESSAGE_ID_CREATE_QUERY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_QUERY: MessageId = MessageId(0x20007f);
/// The header's `D3D11_MESSAGE_ID_LIVE_QUERY_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_QUERY_WIN7: MessageId = MessageId(0x200080);
/// The header's `D3D11_MESSAGE_ID_DESTROY_QUERY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_QUERY: MessageId = MessageId(0x200081);
/// The header's `D3D11_MESSAGE_ID_CREATE_PREDICATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_PREDICATE: MessageId = MessageId(0x200082);
/// The header's `D3D11_MESSAGE_ID_LIVE_PREDICATE_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_PREDICATE_WIN7: MessageId = MessageId(0x200083);
/// The header's `D3D11_MESSAGE_ID_DESTROY_PREDICATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_PREDICATE: MessageId = MessageId(0x200084);
/// The header's `D3D11_MESSAGE_ID_CREATE_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_COUNTER: MessageId = MessageId(0x200085);
/// The header's `D3D11_MESSAGE_ID_DESTROY_COUNTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_COUNTER: MessageId = MessageId(0x200086);
/// The header's `D3D11_MESSAGE_ID_CREATE_COMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_COMMANDLIST: MessageId = MessageId(0x200087);
/// The header's `D3D11_MESSAGE_ID_LIVE_COMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_COMMANDLIST: MessageId = MessageId(0x200088);
/// The header's `D3D11_MESSAGE_ID_DESTROY_COMMANDLIST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_COMMANDLIST: MessageId = MessageId(0x200089);
/// The header's `D3D11_MESSAGE_ID_CREATE_CLASSINSTANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_CLASSINSTANCE: MessageId = MessageId(0x20008a);
/// The header's `D3D11_MESSAGE_ID_LIVE_CLASSINSTANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_CLASSINSTANCE: MessageId = MessageId(0x20008b);
/// The header's `D3D11_MESSAGE_ID_DESTROY_CLASSINSTANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_CLASSINSTANCE: MessageId = MessageId(0x20008c);
/// The header's `D3D11_MESSAGE_ID_CREATE_CLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_CLASSLINKAGE: MessageId = MessageId(0x20008d);
/// The header's `D3D11_MESSAGE_ID_LIVE_CLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_CLASSLINKAGE: MessageId = MessageId(0x20008e);
/// The header's `D3D11_MESSAGE_ID_DESTROY_CLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_CLASSLINKAGE: MessageId = MessageId(0x20008f);
/// The header's `D3D11_MESSAGE_ID_LIVE_DEVICE_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DEVICE_WIN7: MessageId = MessageId(0x200090);
/// The header's `D3D11_MESSAGE_ID_LIVE_OBJECT_SUMMARY_WIN7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_OBJECT_SUMMARY_WIN7: MessageId = MessageId(0x200091);
/// The header's `D3D11_MESSAGE_ID_CREATE_COMPUTESHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_COMPUTESHADER: MessageId = MessageId(0x200092);
/// The header's `D3D11_MESSAGE_ID_LIVE_COMPUTESHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_COMPUTESHADER: MessageId = MessageId(0x200093);
/// The header's `D3D11_MESSAGE_ID_DESTROY_COMPUTESHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_COMPUTESHADER: MessageId = MessageId(0x200094);
/// The header's `D3D11_MESSAGE_ID_CREATE_UNORDEREDACCESSVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_UNORDEREDACCESSVIEW: MessageId = MessageId(0x200095);
/// The header's `D3D11_MESSAGE_ID_LIVE_UNORDEREDACCESSVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_UNORDEREDACCESSVIEW: MessageId = MessageId(0x200096);
/// The header's `D3D11_MESSAGE_ID_DESTROY_UNORDEREDACCESSVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_UNORDEREDACCESSVIEW: MessageId = MessageId(0x200097);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETSHADER_INTERFACES_FEATURELEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETSHADER_INTERFACES_FEATURELEVEL: MessageId =
    MessageId(0x200098);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETSHADER_INTERFACE_COUNT_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETSHADER_INTERFACE_COUNT_MISMATCH: MessageId =
    MessageId(0x200099);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETSHADER_INVALID_INSTANCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETSHADER_INVALID_INSTANCE: MessageId = MessageId(0x20009a);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETSHADER_INVALID_INSTANCE_INDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETSHADER_INVALID_INSTANCE_INDEX: MessageId = MessageId(0x20009b);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETSHADER_INVALID_INSTANCE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETSHADER_INVALID_INSTANCE_TYPE: MessageId = MessageId(0x20009c);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETSHADER_INVALID_INSTANCE_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETSHADER_INVALID_INSTANCE_DATA: MessageId = MessageId(0x20009d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETSHADER_UNBOUND_INSTANCE_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETSHADER_UNBOUND_INSTANCE_DATA: MessageId = MessageId(0x20009e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETSHADER_INSTANCE_DATA_BINDINGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETSHADER_INSTANCE_DATA_BINDINGS: MessageId = MessageId(0x20009f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATESHADER_CLASSLINKAGE_FULL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATESHADER_CLASSLINKAGE_FULL: MessageId = MessageId(0x2000a0);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CHECKFEATURESUPPORT_UNRECOGNIZED_FEATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CHECKFEATURESUPPORT_UNRECOGNIZED_FEATURE: MessageId =
    MessageId(0x2000a1);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CHECKFEATURESUPPORT_MISMATCHED_DATA_SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CHECKFEATURESUPPORT_MISMATCHED_DATA_SIZE: MessageId =
    MessageId(0x2000a2);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CHECKFEATURESUPPORT_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CHECKFEATURESUPPORT_INVALIDARG_RETURN: MessageId =
    MessageId(0x2000a3);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETSHADERRESOURCES_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETSHADERRESOURCES_HAZARD: MessageId = MessageId(0x2000a4);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETCONSTANTBUFFERS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETCONSTANTBUFFERS_HAZARD: MessageId = MessageId(0x2000a5);
/// The header's `D3D11_MESSAGE_ID_CSSETSHADERRESOURCES_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CSSETSHADERRESOURCES_UNBINDDELETINGOBJECT: MessageId =
    MessageId(0x2000a6);
/// The header's `D3D11_MESSAGE_ID_CSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CSSETCONSTANTBUFFERS_UNBINDDELETINGOBJECT: MessageId =
    MessageId(0x2000a7);
/// The header's `D3D11_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDCALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDCALL: MessageId = MessageId(0x2000a8);
/// The header's `D3D11_MESSAGE_ID_CREATECOMPUTESHADER_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOMPUTESHADER_OUTOFMEMORY: MessageId = MessageId(0x2000a9);
/// The header's `D3D11_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDSHADERBYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDSHADERBYTECODE: MessageId =
    MessageId(0x2000aa);
/// The header's `D3D11_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDSHADERTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDSHADERTYPE: MessageId = MessageId(0x2000ab);
/// The header's `D3D11_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDCLASSLINKAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECOMPUTESHADER_INVALIDCLASSLINKAGE: MessageId = MessageId(0x2000ac);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x2000ad);
/// The header's `D3D11_MESSAGE_ID_CSSETCONSTANTBUFFERS_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CSSETCONSTANTBUFFERS_INVALIDBUFFER: MessageId = MessageId(0x2000ae);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId =
    MessageId(0x2000af);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x2000b0);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSGETSHADERRESOURCES_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSGETSHADERRESOURCES_VIEWS_EMPTY: MessageId = MessageId(0x2000b1);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSGETCONSTANTBUFFERS_BUFFERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSGETCONSTANTBUFFERS_BUFFERS_EMPTY: MessageId =
    MessageId(0x2000b2);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSGETSAMPLERS_SAMPLERS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSGETSAMPLERS_SAMPLERS_EMPTY: MessageId = MessageId(0x2000b3);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x2000b4);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEHULLSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEHULLSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x2000b5);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x2000b6);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x2000b7);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DOUBLEFLOATOPSNOTSUPPORTED: MessageId = MessageId(0x2000b8);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x2000b9);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_DOUBLEFLOATOPSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_DOUBLEFLOATOPSNOTSUPPORTED: MessageId =
    MessageId(0x2000ba);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDSTRUCTURESTRIDE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDSTRUCTURESTRIDE: MessageId = MessageId(0x2000bb);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDFLAGS: MessageId = MessageId(0x2000bc);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDRESOURCE: MessageId =
    MessageId(0x2000bd);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDESC: MessageId = MessageId(0x2000be);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDFORMAT: MessageId = MessageId(0x2000bf);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDIMENSIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDIMENSIONS: MessageId =
    MessageId(0x2000c0);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_UNRECOGNIZEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_UNRECOGNIZEDFORMAT: MessageId =
    MessageId(0x2000c1);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_HAZARD: MessageId =
    MessageId(0x2000c2);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_OVERLAPPING_OLD_SLOTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_OVERLAPPING_OLD_SLOTS: MessageId = MessageId(0x2000c3);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_NO_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_NO_OP: MessageId =
    MessageId(0x2000c4);
/// The header's `D3D11_MESSAGE_ID_CSSETUNORDEREDACCESSVIEWS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CSSETUNORDEREDACCESSVIEWS_UNBINDDELETINGOBJECT: MessageId =
    MessageId(0x2000c5);
/// The header's `D3D11_MESSAGE_ID_PSSETUNORDEREDACCESSVIEWS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PSSETUNORDEREDACCESSVIEWS_UNBINDDELETINGOBJECT: MessageId =
    MessageId(0x2000c6);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDARG_RETURN: MessageId =
    MessageId(0x2000c7);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x2000c8);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_TOOMANYOBJECTS: MessageId =
    MessageId(0x2000c9);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSVIEWS_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSVIEWS_HAZARD: MessageId = MessageId(0x2000ca);
/// The header's `D3D11_MESSAGE_ID_CLEARUNORDEREDACCESSVIEW_DENORMFLUSH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARUNORDEREDACCESSVIEW_DENORMFLUSH: MessageId = MessageId(0x2000cb);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSS_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSS_VIEWS_EMPTY: MessageId =
    MessageId(0x2000cc);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSGETUNORDEREDACCESSS_VIEWS_EMPTY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSGETUNORDEREDACCESSS_VIEWS_EMPTY: MessageId =
    MessageId(0x2000cd);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDFLAGS: MessageId = MessageId(0x2000ce);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESESOURCEVIEW_TOOMANYOBJECTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESESOURCEVIEW_TOOMANYOBJECTS: MessageId =
    MessageId(0x2000cf);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISPATCHINDIRECT_INVALID_ARG_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISPATCHINDIRECT_INVALID_ARG_BUFFER: MessageId =
    MessageId(0x2000d0);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISPATCHINDIRECT_OFFSET_UNALIGNED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISPATCHINDIRECT_OFFSET_UNALIGNED: MessageId =
    MessageId(0x2000d1);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISPATCHINDIRECT_OFFSET_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISPATCHINDIRECT_OFFSET_OVERFLOW: MessageId = MessageId(0x2000d2);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETRESOURCEMINLOD_INVALIDCONTEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETRESOURCEMINLOD_INVALIDCONTEXT: MessageId = MessageId(0x2000d3);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETRESOURCEMINLOD_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETRESOURCEMINLOD_INVALIDRESOURCE: MessageId =
    MessageId(0x2000d4);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETRESOURCEMINLOD_INVALIDMINLOD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETRESOURCEMINLOD_INVALIDMINLOD: MessageId = MessageId(0x2000d5);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GETRESOURCEMINLOD_INVALIDCONTEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GETRESOURCEMINLOD_INVALIDCONTEXT: MessageId = MessageId(0x2000d6);
/// The header's `D3D11_MESSAGE_ID_DEVICE_GETRESOURCEMINLOD_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_GETRESOURCEMINLOD_INVALIDRESOURCE: MessageId =
    MessageId(0x2000d7);
/// The header's `D3D11_MESSAGE_ID_OMSETDEPTHSTENCIL_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OMSETDEPTHSTENCIL_UNBINDDELETINGOBJECT: MessageId = MessageId(0x2000d8);
/// The header's `D3D11_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_DEPTH_READONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_DEPTH_READONLY: MessageId = MessageId(0x2000d9);
/// The header's `D3D11_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_STENCIL_READONLY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARDEPTHSTENCILVIEW_STENCIL_READONLY: MessageId = MessageId(0x2000da);
/// The header's `D3D11_MESSAGE_ID_CHECKFEATURESUPPORT_FORMAT_DEPRECATED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKFEATURESUPPORT_FORMAT_DEPRECATED: MessageId = MessageId(0x2000db);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_RETURN_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_RETURN_TYPE_MISMATCH: MessageId =
    MessageId(0x2000dc);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_NOT_SET: MessageId = MessageId(0x2000dd);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_UNORDEREDACCESSVIEW_RENDERTARGETVIEW_OVERLAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_UNORDEREDACCESSVIEW_RENDERTARGETVIEW_OVERLAP: MessageId =
    MessageId(0x2000de);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_DIMENSION_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_DIMENSION_MISMATCH: MessageId =
    MessageId(0x2000df);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_APPEND_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_APPEND_UNSUPPORTED: MessageId =
    MessageId(0x2000e0);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMICS_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMICS_UNSUPPORTED: MessageId =
    MessageId(0x2000e1);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_STRUCTURE_STRIDE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_STRUCTURE_STRIDE_MISMATCH: MessageId =
    MessageId(0x2000e2);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_BUFFER_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_BUFFER_TYPE_MISMATCH: MessageId =
    MessageId(0x2000e3);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_RAW_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_RAW_UNSUPPORTED: MessageId =
    MessageId(0x2000e4);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_FORMAT_LD_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_FORMAT_LD_UNSUPPORTED: MessageId =
    MessageId(0x2000e5);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_FORMAT_STORE_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_FORMAT_STORE_UNSUPPORTED: MessageId =
    MessageId(0x2000e6);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_ADD_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_ADD_UNSUPPORTED: MessageId =
    MessageId(0x2000e7);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_BITWISE_OPS_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_BITWISE_OPS_UNSUPPORTED: MessageId =
    MessageId(0x2000e8);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_CMPSTORE_CMPEXCHANGE_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_CMPSTORE_CMPEXCHANGE_UNSUPPORTED:
    MessageId = MessageId(0x2000e9);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_EXCHANGE_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_EXCHANGE_UNSUPPORTED: MessageId =
    MessageId(0x2000ea);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_SIGNED_MINMAX_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_SIGNED_MINMAX_UNSUPPORTED: MessageId =
    MessageId(0x2000eb);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_UNSIGNED_MINMAX_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_ATOMIC_UNSIGNED_MINMAX_UNSUPPORTED:
    MessageId = MessageId(0x2000ec);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISPATCH_BOUND_RESOURCE_MAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISPATCH_BOUND_RESOURCE_MAPPED: MessageId = MessageId(0x2000ed);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISPATCH_THREADGROUPCOUNT_OVERFLOW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISPATCH_THREADGROUPCOUNT_OVERFLOW: MessageId =
    MessageId(0x2000ee);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISPATCH_THREADGROUPCOUNT_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISPATCH_THREADGROUPCOUNT_ZERO: MessageId = MessageId(0x2000ef);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADERRESOURCEVIEW_STRUCTURE_STRIDE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADERRESOURCEVIEW_STRUCTURE_STRIDE_MISMATCH: MessageId =
    MessageId(0x2000f0);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADERRESOURCEVIEW_BUFFER_TYPE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADERRESOURCEVIEW_BUFFER_TYPE_MISMATCH: MessageId =
    MessageId(0x2000f1);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADERRESOURCEVIEW_RAW_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADERRESOURCEVIEW_RAW_UNSUPPORTED: MessageId =
    MessageId(0x2000f2);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISPATCH_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISPATCH_UNSUPPORTED: MessageId = MessageId(0x2000f3);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISPATCHINDIRECT_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISPATCHINDIRECT_UNSUPPORTED: MessageId = MessageId(0x2000f4);
/// The header's `D3D11_MESSAGE_ID_COPYSTRUCTURECOUNT_INVALIDOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSTRUCTURECOUNT_INVALIDOFFSET: MessageId = MessageId(0x2000f5);
/// The header's `D3D11_MESSAGE_ID_COPYSTRUCTURECOUNT_LARGEOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSTRUCTURECOUNT_LARGEOFFSET: MessageId = MessageId(0x2000f6);
/// The header's `D3D11_MESSAGE_ID_COPYSTRUCTURECOUNT_INVALIDDESTINATIONSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSTRUCTURECOUNT_INVALIDDESTINATIONSTATE: MessageId =
    MessageId(0x2000f7);
/// The header's `D3D11_MESSAGE_ID_COPYSTRUCTURECOUNT_INVALIDSOURCESTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSTRUCTURECOUNT_INVALIDSOURCESTATE: MessageId = MessageId(0x2000f8);
/// The header's `D3D11_MESSAGE_ID_CHECKFORMATSUPPORT_FORMAT_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKFORMATSUPPORT_FORMAT_NOT_SUPPORTED: MessageId = MessageId(0x2000f9);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSVIEWS_INVALIDVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSVIEWS_INVALIDVIEW: MessageId =
    MessageId(0x2000fa);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSVIEWS_INVALIDOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSVIEWS_INVALIDOFFSET: MessageId =
    MessageId(0x2000fb);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSVIEWS_TOOMANYVIEWS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CSSETUNORDEREDACCESSVIEWS_TOOMANYVIEWS: MessageId =
    MessageId(0x2000fc);
/// The header's `D3D11_MESSAGE_ID_CLEARUNORDEREDACCESSVIEWFLOAT_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARUNORDEREDACCESSVIEWFLOAT_INVALIDFORMAT: MessageId =
    MessageId(0x2000fd);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_COUNTER_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_COUNTER_UNSUPPORTED: MessageId =
    MessageId(0x2000fe);
/// The header's `D3D11_MESSAGE_ID_REF_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_WARNING: MessageId = MessageId(0x2000ff);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_PIXEL_SHADER_WITHOUT_RTV_OR_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_PIXEL_SHADER_WITHOUT_RTV_OR_DSV: MessageId =
    MessageId(0x200100);
/// The header's `D3D11_MESSAGE_ID_SHADER_ABORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SHADER_ABORT: MessageId = MessageId(0x200101);
/// The header's `D3D11_MESSAGE_ID_SHADER_MESSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SHADER_MESSAGE: MessageId = MessageId(0x200102);
/// The header's `D3D11_MESSAGE_ID_SHADER_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SHADER_ERROR: MessageId = MessageId(0x200103);
/// The header's `D3D11_MESSAGE_ID_OFFERRESOURCES_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OFFERRESOURCES_INVALIDRESOURCE: MessageId = MessageId(0x200104);
/// The header's `D3D11_MESSAGE_ID_HSSETSAMPLERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_HSSETSAMPLERS_UNBINDDELETINGOBJECT: MessageId = MessageId(0x200105);
/// The header's `D3D11_MESSAGE_ID_DSSETSAMPLERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DSSETSAMPLERS_UNBINDDELETINGOBJECT: MessageId = MessageId(0x200106);
/// The header's `D3D11_MESSAGE_ID_CSSETSAMPLERS_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CSSETSAMPLERS_UNBINDDELETINGOBJECT: MessageId = MessageId(0x200107);
/// The header's `D3D11_MESSAGE_ID_HSSETSHADER_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_HSSETSHADER_UNBINDDELETINGOBJECT: MessageId = MessageId(0x200108);
/// The header's `D3D11_MESSAGE_ID_DSSETSHADER_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DSSETSHADER_UNBINDDELETINGOBJECT: MessageId = MessageId(0x200109);
/// The header's `D3D11_MESSAGE_ID_CSSETSHADER_UNBINDDELETINGOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CSSETSHADER_UNBINDDELETINGOBJECT: MessageId = MessageId(0x20010a);
/// The header's `D3D11_MESSAGE_ID_ENQUEUESETEVENT_INVALIDARG_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENQUEUESETEVENT_INVALIDARG_RETURN: MessageId = MessageId(0x20010b);
/// The header's `D3D11_MESSAGE_ID_ENQUEUESETEVENT_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENQUEUESETEVENT_OUTOFMEMORY_RETURN: MessageId = MessageId(0x20010c);
/// The header's `D3D11_MESSAGE_ID_ENQUEUESETEVENT_ACCESSDENIED_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENQUEUESETEVENT_ACCESSDENIED_RETURN: MessageId = MessageId(0x20010d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_NUMUAVS_INVALIDRANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_NUMUAVS_INVALIDRANGE:
    MessageId = MessageId(0x20010e);
/// The header's `D3D11_MESSAGE_ID_USE_OF_ZERO_REFCOUNT_OBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_USE_OF_ZERO_REFCOUNT_OBJECT: MessageId = MessageId(0x20010f);
/// The header's `D3D11_MESSAGE_ID_D3D11_MESSAGES_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_MESSAGES_END: MessageId = MessageId(0x200110);
/// The header's `D3D11_MESSAGE_ID_D3D11_1_MESSAGES_START`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_1_MESSAGES_START: MessageId = MessageId(0x300000);
/// The header's `D3D11_MESSAGE_ID_CREATE_VIDEODECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_VIDEODECODER: MessageId = MessageId(0x300001);
/// The header's `D3D11_MESSAGE_ID_CREATE_VIDEOPROCESSORENUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_VIDEOPROCESSORENUM: MessageId = MessageId(0x300002);
/// The header's `D3D11_MESSAGE_ID_CREATE_VIDEOPROCESSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_VIDEOPROCESSOR: MessageId = MessageId(0x300003);
/// The header's `D3D11_MESSAGE_ID_CREATE_DECODEROUTPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_DECODEROUTPUTVIEW: MessageId = MessageId(0x300004);
/// The header's `D3D11_MESSAGE_ID_CREATE_PROCESSORINPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_PROCESSORINPUTVIEW: MessageId = MessageId(0x300005);
/// The header's `D3D11_MESSAGE_ID_CREATE_PROCESSOROUTPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_PROCESSOROUTPUTVIEW: MessageId = MessageId(0x300006);
/// The header's `D3D11_MESSAGE_ID_CREATE_DEVICECONTEXTSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_DEVICECONTEXTSTATE: MessageId = MessageId(0x300007);
/// The header's `D3D11_MESSAGE_ID_LIVE_VIDEODECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_VIDEODECODER: MessageId = MessageId(0x300008);
/// The header's `D3D11_MESSAGE_ID_LIVE_VIDEOPROCESSORENUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_VIDEOPROCESSORENUM: MessageId = MessageId(0x300009);
/// The header's `D3D11_MESSAGE_ID_LIVE_VIDEOPROCESSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_VIDEOPROCESSOR: MessageId = MessageId(0x30000a);
/// The header's `D3D11_MESSAGE_ID_LIVE_DECODEROUTPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DECODEROUTPUTVIEW: MessageId = MessageId(0x30000b);
/// The header's `D3D11_MESSAGE_ID_LIVE_PROCESSORINPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_PROCESSORINPUTVIEW: MessageId = MessageId(0x30000c);
/// The header's `D3D11_MESSAGE_ID_LIVE_PROCESSOROUTPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_PROCESSOROUTPUTVIEW: MessageId = MessageId(0x30000d);
/// The header's `D3D11_MESSAGE_ID_LIVE_DEVICECONTEXTSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_DEVICECONTEXTSTATE: MessageId = MessageId(0x30000e);
/// The header's `D3D11_MESSAGE_ID_DESTROY_VIDEODECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_VIDEODECODER: MessageId = MessageId(0x30000f);
/// The header's `D3D11_MESSAGE_ID_DESTROY_VIDEOPROCESSORENUM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_VIDEOPROCESSORENUM: MessageId = MessageId(0x300010);
/// The header's `D3D11_MESSAGE_ID_DESTROY_VIDEOPROCESSOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_VIDEOPROCESSOR: MessageId = MessageId(0x300011);
/// The header's `D3D11_MESSAGE_ID_DESTROY_DECODEROUTPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_DECODEROUTPUTVIEW: MessageId = MessageId(0x300012);
/// The header's `D3D11_MESSAGE_ID_DESTROY_PROCESSORINPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_PROCESSORINPUTVIEW: MessageId = MessageId(0x300013);
/// The header's `D3D11_MESSAGE_ID_DESTROY_PROCESSOROUTPUTVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_PROCESSOROUTPUTVIEW: MessageId = MessageId(0x300014);
/// The header's `D3D11_MESSAGE_ID_DESTROY_DEVICECONTEXTSTATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_DEVICECONTEXTSTATE: MessageId = MessageId(0x300015);
/// The header's `D3D11_MESSAGE_ID_CREATEDEVICECONTEXTSTATE_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEVICECONTEXTSTATE_INVALIDFLAGS: MessageId = MessageId(0x300016);
/// The header's `D3D11_MESSAGE_ID_CREATEDEVICECONTEXTSTATE_INVALIDFEATURELEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEVICECONTEXTSTATE_INVALIDFEATURELEVEL: MessageId =
    MessageId(0x300017);
/// The header's `D3D11_MESSAGE_ID_CREATEDEVICECONTEXTSTATE_FEATURELEVELS_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEVICECONTEXTSTATE_FEATURELEVELS_NOT_SUPPORTED: MessageId =
    MessageId(0x300018);
/// The header's `D3D11_MESSAGE_ID_CREATEDEVICECONTEXTSTATE_INVALIDREFIID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEVICECONTEXTSTATE_INVALIDREFIID: MessageId = MessageId(0x300019);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DISCARDVIEW_INVALIDVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DISCARDVIEW_INVALIDVIEW: MessageId = MessageId(0x30001a);
/// The header's `D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION1_INVALIDCOPYFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION1_INVALIDCOPYFLAGS: MessageId = MessageId(0x30001b);
/// The header's `D3D11_MESSAGE_ID_UPDATESUBRESOURCE1_INVALIDCOPYFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UPDATESUBRESOURCE1_INVALIDCOPYFLAGS: MessageId = MessageId(0x30001c);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDFORCEDSAMPLECOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALIDFORCEDSAMPLECOUNT: MessageId =
    MessageId(0x30001d);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODER_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODER_OUTOFMEMORY_RETURN: MessageId = MessageId(0x30001e);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODER_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODER_NULLPARAM: MessageId = MessageId(0x30001f);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODER_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODER_INVALIDFORMAT: MessageId = MessageId(0x300020);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODER_ZEROWIDTHHEIGHT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODER_ZEROWIDTHHEIGHT: MessageId = MessageId(0x300021);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODER_DRIVER_INVALIDBUFFERSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODER_DRIVER_INVALIDBUFFERSIZE: MessageId =
    MessageId(0x300022);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODER_DRIVER_INVALIDBUFFERUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODER_DRIVER_INVALIDBUFFERUSAGE: MessageId =
    MessageId(0x300023);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERPROFILECOUNT_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERPROFILECOUNT_OUTOFMEMORY: MessageId = MessageId(0x300024);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERPROFILE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERPROFILE_NULLPARAM: MessageId = MessageId(0x300025);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERPROFILE_INVALIDINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERPROFILE_INVALIDINDEX: MessageId = MessageId(0x300026);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERPROFILE_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERPROFILE_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x300027);
/// The header's `D3D11_MESSAGE_ID_CHECKVIDEODECODERFORMAT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKVIDEODECODERFORMAT_NULLPARAM: MessageId = MessageId(0x300028);
/// The header's `D3D11_MESSAGE_ID_CHECKVIDEODECODERFORMAT_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKVIDEODECODERFORMAT_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x300029);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERCONFIGCOUNT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERCONFIGCOUNT_NULLPARAM: MessageId = MessageId(0x30002a);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERCONFIGCOUNT_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERCONFIGCOUNT_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x30002b);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERCONFIG_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERCONFIG_NULLPARAM: MessageId = MessageId(0x30002c);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERCONFIG_INVALIDINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERCONFIG_INVALIDINDEX: MessageId = MessageId(0x30002d);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERCONFIG_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERCONFIG_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x30002e);
/// The header's `D3D11_MESSAGE_ID_GETDECODERCREATIONPARAMS_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDECODERCREATIONPARAMS_NULLPARAM: MessageId = MessageId(0x30002f);
/// The header's `D3D11_MESSAGE_ID_GETDECODERDRIVERHANDLE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDECODERDRIVERHANDLE_NULLPARAM: MessageId = MessageId(0x300030);
/// The header's `D3D11_MESSAGE_ID_GETDECODERBUFFER_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDECODERBUFFER_NULLPARAM: MessageId = MessageId(0x300031);
/// The header's `D3D11_MESSAGE_ID_GETDECODERBUFFER_INVALIDBUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDECODERBUFFER_INVALIDBUFFER: MessageId = MessageId(0x300032);
/// The header's `D3D11_MESSAGE_ID_GETDECODERBUFFER_INVALIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDECODERBUFFER_INVALIDTYPE: MessageId = MessageId(0x300033);
/// The header's `D3D11_MESSAGE_ID_GETDECODERBUFFER_LOCKED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDECODERBUFFER_LOCKED: MessageId = MessageId(0x300034);
/// The header's `D3D11_MESSAGE_ID_RELEASEDECODERBUFFER_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RELEASEDECODERBUFFER_NULLPARAM: MessageId = MessageId(0x300035);
/// The header's `D3D11_MESSAGE_ID_RELEASEDECODERBUFFER_INVALIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RELEASEDECODERBUFFER_INVALIDTYPE: MessageId = MessageId(0x300036);
/// The header's `D3D11_MESSAGE_ID_RELEASEDECODERBUFFER_NOTLOCKED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RELEASEDECODERBUFFER_NOTLOCKED: MessageId = MessageId(0x300037);
/// The header's `D3D11_MESSAGE_ID_DECODERBEGINFRAME_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERBEGINFRAME_NULLPARAM: MessageId = MessageId(0x300038);
/// The header's `D3D11_MESSAGE_ID_DECODERBEGINFRAME_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERBEGINFRAME_HAZARD: MessageId = MessageId(0x300039);
/// The header's `D3D11_MESSAGE_ID_DECODERENDFRAME_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERENDFRAME_NULLPARAM: MessageId = MessageId(0x30003a);
/// The header's `D3D11_MESSAGE_ID_SUBMITDECODERBUFFERS_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SUBMITDECODERBUFFERS_NULLPARAM: MessageId = MessageId(0x30003b);
/// The header's `D3D11_MESSAGE_ID_SUBMITDECODERBUFFERS_INVALIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SUBMITDECODERBUFFERS_INVALIDTYPE: MessageId = MessageId(0x30003c);
/// The header's `D3D11_MESSAGE_ID_DECODEREXTENSION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODEREXTENSION_NULLPARAM: MessageId = MessageId(0x30003d);
/// The header's `D3D11_MESSAGE_ID_DECODEREXTENSION_INVALIDRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODEREXTENSION_INVALIDRESOURCE: MessageId = MessageId(0x30003e);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x30003f);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_NULLPARAM: MessageId =
    MessageId(0x300040);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDFRAMEFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDFRAMEFORMAT: MessageId =
    MessageId(0x300041);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDUSAGE: MessageId =
    MessageId(0x300042);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDINPUTFRAMERATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDINPUTFRAMERATE: MessageId =
    MessageId(0x300043);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDOUTPUTFRAMERATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDOUTPUTFRAMERATE: MessageId =
    MessageId(0x300044);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDWIDTHHEIGHT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORENUMERATOR_INVALIDWIDTHHEIGHT: MessageId =
    MessageId(0x300045);
/// The header's `D3D11_MESSAGE_ID_GETVIDEOPROCESSORCONTENTDESC_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEOPROCESSORCONTENTDESC_NULLPARAM: MessageId = MessageId(0x300046);
/// The header's `D3D11_MESSAGE_ID_CHECKVIDEOPROCESSORFORMAT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKVIDEOPROCESSORFORMAT_NULLPARAM: MessageId = MessageId(0x300047);
/// The header's `D3D11_MESSAGE_ID_GETVIDEOPROCESSORCAPS_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEOPROCESSORCAPS_NULLPARAM: MessageId = MessageId(0x300048);
/// The header's `D3D11_MESSAGE_ID_GETVIDEOPROCESSORRATECONVERSIONCAPS_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEOPROCESSORRATECONVERSIONCAPS_NULLPARAM: MessageId =
    MessageId(0x300049);
/// The header's `D3D11_MESSAGE_ID_GETVIDEOPROCESSORRATECONVERSIONCAPS_INVALIDINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEOPROCESSORRATECONVERSIONCAPS_INVALIDINDEX: MessageId =
    MessageId(0x30004a);
/// The header's `D3D11_MESSAGE_ID_GETVIDEOPROCESSORCUSTOMRATE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEOPROCESSORCUSTOMRATE_NULLPARAM: MessageId = MessageId(0x30004b);
/// The header's `D3D11_MESSAGE_ID_GETVIDEOPROCESSORCUSTOMRATE_INVALIDINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEOPROCESSORCUSTOMRATE_INVALIDINDEX: MessageId =
    MessageId(0x30004c);
/// The header's `D3D11_MESSAGE_ID_GETVIDEOPROCESSORFILTERRANGE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEOPROCESSORFILTERRANGE_NULLPARAM: MessageId = MessageId(0x30004d);
/// The header's `D3D11_MESSAGE_ID_GETVIDEOPROCESSORFILTERRANGE_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEOPROCESSORFILTERRANGE_UNSUPPORTED: MessageId =
    MessageId(0x30004e);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOR_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOR_OUTOFMEMORY_RETURN: MessageId = MessageId(0x30004f);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOR_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOR_NULLPARAM: MessageId = MessageId(0x300050);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTTARGETRECT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTTARGETRECT_NULLPARAM: MessageId =
    MessageId(0x300051);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTBACKGROUNDCOLOR_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTBACKGROUNDCOLOR_NULLPARAM: MessageId =
    MessageId(0x300052);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTBACKGROUNDCOLOR_INVALIDALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTBACKGROUNDCOLOR_INVALIDALPHA: MessageId =
    MessageId(0x300053);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCOLORSPACE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCOLORSPACE_NULLPARAM: MessageId =
    MessageId(0x300054);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTALPHAFILLMODE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTALPHAFILLMODE_NULLPARAM: MessageId =
    MessageId(0x300055);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTALPHAFILLMODE_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTALPHAFILLMODE_UNSUPPORTED: MessageId =
    MessageId(0x300056);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTALPHAFILLMODE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTALPHAFILLMODE_INVALIDSTREAM: MessageId =
    MessageId(0x300057);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTALPHAFILLMODE_INVALIDFILLMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTALPHAFILLMODE_INVALIDFILLMODE: MessageId =
    MessageId(0x300058);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCONSTRICTION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCONSTRICTION_NULLPARAM: MessageId =
    MessageId(0x300059);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTSTEREOMODE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTSTEREOMODE_NULLPARAM: MessageId =
    MessageId(0x30005a);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTSTEREOMODE_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTSTEREOMODE_UNSUPPORTED: MessageId =
    MessageId(0x30005b);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTEXTENSION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTEXTENSION_NULLPARAM: MessageId =
    MessageId(0x30005c);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTTARGETRECT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTTARGETRECT_NULLPARAM: MessageId =
    MessageId(0x30005d);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTBACKGROUNDCOLOR_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTBACKGROUNDCOLOR_NULLPARAM: MessageId =
    MessageId(0x30005e);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTCOLORSPACE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTCOLORSPACE_NULLPARAM: MessageId =
    MessageId(0x30005f);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTALPHAFILLMODE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTALPHAFILLMODE_NULLPARAM: MessageId =
    MessageId(0x300060);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTCONSTRICTION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTCONSTRICTION_NULLPARAM: MessageId =
    MessageId(0x300061);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCONSTRICTION_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCONSTRICTION_UNSUPPORTED: MessageId =
    MessageId(0x300062);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCONSTRICTION_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCONSTRICTION_INVALIDSIZE: MessageId =
    MessageId(0x300063);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTSTEREOMODE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTSTEREOMODE_NULLPARAM: MessageId =
    MessageId(0x300064);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTEXTENSION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTEXTENSION_NULLPARAM: MessageId =
    MessageId(0x300065);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFRAMEFORMAT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFRAMEFORMAT_NULLPARAM: MessageId =
    MessageId(0x300066);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFRAMEFORMAT_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFRAMEFORMAT_INVALIDFORMAT: MessageId =
    MessageId(0x300067);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFRAMEFORMAT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFRAMEFORMAT_INVALIDSTREAM: MessageId =
    MessageId(0x300068);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMCOLORSPACE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMCOLORSPACE_NULLPARAM: MessageId =
    MessageId(0x300069);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMCOLORSPACE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMCOLORSPACE_INVALIDSTREAM: MessageId =
    MessageId(0x30006a);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMOUTPUTRATE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMOUTPUTRATE_NULLPARAM: MessageId =
    MessageId(0x30006b);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMOUTPUTRATE_INVALIDRATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMOUTPUTRATE_INVALIDRATE: MessageId =
    MessageId(0x30006c);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMOUTPUTRATE_INVALIDFLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMOUTPUTRATE_INVALIDFLAG: MessageId =
    MessageId(0x30006d);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMOUTPUTRATE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMOUTPUTRATE_INVALIDSTREAM: MessageId =
    MessageId(0x30006e);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSOURCERECT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSOURCERECT_NULLPARAM: MessageId =
    MessageId(0x30006f);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSOURCERECT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSOURCERECT_INVALIDSTREAM: MessageId =
    MessageId(0x300070);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSOURCERECT_INVALIDRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSOURCERECT_INVALIDRECT: MessageId =
    MessageId(0x300071);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMDESTRECT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMDESTRECT_NULLPARAM: MessageId =
    MessageId(0x300072);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMDESTRECT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMDESTRECT_INVALIDSTREAM: MessageId =
    MessageId(0x300073);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMDESTRECT_INVALIDRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMDESTRECT_INVALIDRECT: MessageId =
    MessageId(0x300074);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMALPHA_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMALPHA_NULLPARAM: MessageId = MessageId(0x300075);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMALPHA_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMALPHA_INVALIDSTREAM: MessageId =
    MessageId(0x300076);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMALPHA_INVALIDALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMALPHA_INVALIDALPHA: MessageId =
    MessageId(0x300077);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPALETTE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPALETTE_NULLPARAM: MessageId =
    MessageId(0x300078);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPALETTE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPALETTE_INVALIDSTREAM: MessageId =
    MessageId(0x300079);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPALETTE_INVALIDCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPALETTE_INVALIDCOUNT: MessageId =
    MessageId(0x30007a);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPALETTE_INVALIDALPHA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPALETTE_INVALIDALPHA: MessageId =
    MessageId(0x30007b);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPIXELASPECTRATIO_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPIXELASPECTRATIO_NULLPARAM: MessageId =
    MessageId(0x30007c);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPIXELASPECTRATIO_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPIXELASPECTRATIO_INVALIDSTREAM: MessageId =
    MessageId(0x30007d);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPIXELASPECTRATIO_INVALIDRATIO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPIXELASPECTRATIO_INVALIDRATIO: MessageId =
    MessageId(0x30007e);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMLUMAKEY_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMLUMAKEY_NULLPARAM: MessageId =
    MessageId(0x30007f);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMLUMAKEY_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMLUMAKEY_INVALIDSTREAM: MessageId =
    MessageId(0x300080);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMLUMAKEY_INVALIDRANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMLUMAKEY_INVALIDRANGE: MessageId =
    MessageId(0x300081);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMLUMAKEY_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMLUMAKEY_UNSUPPORTED: MessageId =
    MessageId(0x300082);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_NULLPARAM: MessageId =
    MessageId(0x300083);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_INVALIDSTREAM: MessageId =
    MessageId(0x300084);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_UNSUPPORTED: MessageId =
    MessageId(0x300085);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_FLIPUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_FLIPUNSUPPORTED: MessageId =
    MessageId(0x300086);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_MONOOFFSETUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_MONOOFFSETUNSUPPORTED: MessageId =
    MessageId(0x300087);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_FORMATUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_FORMATUNSUPPORTED: MessageId =
    MessageId(0x300088);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMSTEREOFORMAT_INVALIDFORMAT: MessageId =
    MessageId(0x300089);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMAUTOPROCESSINGMODE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMAUTOPROCESSINGMODE_NULLPARAM: MessageId =
    MessageId(0x30008a);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMAUTOPROCESSINGMODE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMAUTOPROCESSINGMODE_INVALIDSTREAM: MessageId =
    MessageId(0x30008b);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_NULLPARAM: MessageId = MessageId(0x30008c);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_INVALIDSTREAM: MessageId =
    MessageId(0x30008d);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_INVALIDFILTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_INVALIDFILTER: MessageId =
    MessageId(0x30008e);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_UNSUPPORTED: MessageId =
    MessageId(0x30008f);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_INVALIDLEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMFILTER_INVALIDLEVEL: MessageId =
    MessageId(0x300090);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMEXTENSION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMEXTENSION_NULLPARAM: MessageId =
    MessageId(0x300091);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMEXTENSION_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMEXTENSION_INVALIDSTREAM: MessageId =
    MessageId(0x300092);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMFRAMEFORMAT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMFRAMEFORMAT_NULLPARAM: MessageId =
    MessageId(0x300093);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMCOLORSPACE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMCOLORSPACE_NULLPARAM: MessageId =
    MessageId(0x300094);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMOUTPUTRATE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMOUTPUTRATE_NULLPARAM: MessageId =
    MessageId(0x300095);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMSOURCERECT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMSOURCERECT_NULLPARAM: MessageId =
    MessageId(0x300096);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMDESTRECT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMDESTRECT_NULLPARAM: MessageId =
    MessageId(0x300097);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMALPHA_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMALPHA_NULLPARAM: MessageId = MessageId(0x300098);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMPALETTE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMPALETTE_NULLPARAM: MessageId =
    MessageId(0x300099);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMPIXELASPECTRATIO_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMPIXELASPECTRATIO_NULLPARAM: MessageId =
    MessageId(0x30009a);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMLUMAKEY_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMLUMAKEY_NULLPARAM: MessageId =
    MessageId(0x30009b);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMSTEREOFORMAT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMSTEREOFORMAT_NULLPARAM: MessageId =
    MessageId(0x30009c);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMAUTOPROCESSINGMODE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMAUTOPROCESSINGMODE_NULLPARAM: MessageId =
    MessageId(0x30009d);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMFILTER_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMFILTER_NULLPARAM: MessageId = MessageId(0x30009e);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMEXTENSION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMEXTENSION_NULLPARAM: MessageId =
    MessageId(0x30009f);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMEXTENSION_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMEXTENSION_INVALIDSTREAM: MessageId =
    MessageId(0x3000a0);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_NULLPARAM: MessageId = MessageId(0x3000a1);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDSTREAMCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDSTREAMCOUNT: MessageId = MessageId(0x3000a2);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_TARGETRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_TARGETRECT: MessageId = MessageId(0x3000a3);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDOUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDOUTPUT: MessageId = MessageId(0x3000a4);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDPASTFRAMES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDPASTFRAMES: MessageId = MessageId(0x3000a5);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDFUTUREFRAMES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDFUTUREFRAMES: MessageId = MessageId(0x3000a6);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDSOURCERECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDSOURCERECT: MessageId = MessageId(0x3000a7);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDDESTRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDDESTRECT: MessageId = MessageId(0x3000a8);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDINPUTRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDINPUTRESOURCE: MessageId = MessageId(0x3000a9);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDARRAYSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDARRAYSIZE: MessageId = MessageId(0x3000aa);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDARRAY: MessageId = MessageId(0x3000ab);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_RIGHTEXPECTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_RIGHTEXPECTED: MessageId = MessageId(0x3000ac);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_RIGHTNOTEXPECTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_RIGHTNOTEXPECTED: MessageId = MessageId(0x3000ad);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_STEREONOTENABLED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_STEREONOTENABLED: MessageId = MessageId(0x3000ae);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDRIGHTRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INVALIDRIGHTRESOURCE: MessageId = MessageId(0x3000af);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_NOSTEREOSTREAMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_NOSTEREOSTREAMS: MessageId = MessageId(0x3000b0);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INPUTHAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_INPUTHAZARD: MessageId = MessageId(0x3000b1);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_OUTPUTHAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORBLT_OUTPUTHAZARD: MessageId = MessageId(0x3000b2);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x3000b3);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_NULLPARAM: MessageId = MessageId(0x3000b4);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDTYPE: MessageId =
    MessageId(0x3000b5);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDBIND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDBIND: MessageId =
    MessageId(0x3000b6);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_UNSUPPORTEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_UNSUPPORTEDFORMAT: MessageId =
    MessageId(0x3000b7);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDMIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDMIP: MessageId = MessageId(0x3000b8);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_UNSUPPORTEMIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_UNSUPPORTEMIP: MessageId =
    MessageId(0x3000b9);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDARRAYSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDARRAYSIZE: MessageId =
    MessageId(0x3000ba);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDARRAY: MessageId =
    MessageId(0x3000bb);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDDIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEODECODEROUTPUTVIEW_INVALIDDIMENSION: MessageId =
    MessageId(0x3000bc);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x3000bd);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_NULLPARAM: MessageId = MessageId(0x3000be);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDTYPE: MessageId =
    MessageId(0x3000bf);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDBIND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDBIND: MessageId =
    MessageId(0x3000c0);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDMISC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDMISC: MessageId =
    MessageId(0x3000c1);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDUSAGE: MessageId =
    MessageId(0x3000c2);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDFORMAT: MessageId =
    MessageId(0x3000c3);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDFOURCC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDFOURCC: MessageId =
    MessageId(0x3000c4);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDMIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDMIP: MessageId =
    MessageId(0x3000c5);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_UNSUPPORTEDMIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_UNSUPPORTEDMIP: MessageId =
    MessageId(0x3000c6);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDARRAYSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDARRAYSIZE: MessageId =
    MessageId(0x3000c7);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDARRAY: MessageId =
    MessageId(0x3000c8);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDDIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDDIMENSION: MessageId =
    MessageId(0x3000c9);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x3000ca);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_NULLPARAM: MessageId =
    MessageId(0x3000cb);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDTYPE: MessageId =
    MessageId(0x3000cc);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDBIND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDBIND: MessageId =
    MessageId(0x3000cd);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDFORMAT: MessageId =
    MessageId(0x3000ce);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDMIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDMIP: MessageId =
    MessageId(0x3000cf);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_UNSUPPORTEDMIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_UNSUPPORTEDMIP: MessageId =
    MessageId(0x3000d0);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_UNSUPPORTEDARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_UNSUPPORTEDARRAY: MessageId =
    MessageId(0x3000d1);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDARRAY: MessageId =
    MessageId(0x3000d2);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDDIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDDIMENSION: MessageId =
    MessageId(0x3000d3);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INVALID_USE_OF_FORCED_SAMPLE_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INVALID_USE_OF_FORCED_SAMPLE_COUNT: MessageId =
    MessageId(0x3000d4);
/// The header's `D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDLOGICOPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBLENDSTATE_INVALIDLOGICOPS: MessageId = MessageId(0x3000d5);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDARRAYWITHDECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDDARRAYWITHDECODER: MessageId =
    MessageId(0x3000d6);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDARRAYWITHDECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDDARRAYWITHDECODER: MessageId =
    MessageId(0x3000d7);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDARRAYWITHDECODER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDDARRAYWITHDECODER: MessageId =
    MessageId(0x3000d8);
/// The header's `D3D11_MESSAGE_ID_DEVICE_LOCKEDOUT_INTERFACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_LOCKEDOUT_INTERFACE: MessageId = MessageId(0x3000d9);
/// The header's `D3D11_MESSAGE_ID_REF_WARNING_ATOMIC_INCONSISTENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_WARNING_ATOMIC_INCONSISTENT: MessageId = MessageId(0x3000da);
/// The header's `D3D11_MESSAGE_ID_REF_WARNING_READING_UNINITIALIZED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_WARNING_READING_UNINITIALIZED_RESOURCE: MessageId =
    MessageId(0x3000db);
/// The header's `D3D11_MESSAGE_ID_REF_WARNING_RAW_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_WARNING_RAW_HAZARD: MessageId = MessageId(0x3000dc);
/// The header's `D3D11_MESSAGE_ID_REF_WARNING_WAR_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_WARNING_WAR_HAZARD: MessageId = MessageId(0x3000dd);
/// The header's `D3D11_MESSAGE_ID_REF_WARNING_WAW_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_REF_WARNING_WAW_HAZARD: MessageId = MessageId(0x3000de);
/// The header's `D3D11_MESSAGE_ID_CREATECRYPTOSESSION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECRYPTOSESSION_NULLPARAM: MessageId = MessageId(0x3000df);
/// The header's `D3D11_MESSAGE_ID_CREATECRYPTOSESSION_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATECRYPTOSESSION_OUTOFMEMORY_RETURN: MessageId = MessageId(0x3000e0);
/// The header's `D3D11_MESSAGE_ID_GETCRYPTOTYPE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCRYPTOTYPE_NULLPARAM: MessageId = MessageId(0x3000e1);
/// The header's `D3D11_MESSAGE_ID_GETDECODERPROFILE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDECODERPROFILE_NULLPARAM: MessageId = MessageId(0x3000e2);
/// The header's `D3D11_MESSAGE_ID_GETCRYPTOSESSIONCERTIFICATESIZE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCRYPTOSESSIONCERTIFICATESIZE_NULLPARAM: MessageId =
    MessageId(0x3000e3);
/// The header's `D3D11_MESSAGE_ID_GETCRYPTOSESSIONCERTIFICATE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCRYPTOSESSIONCERTIFICATE_NULLPARAM: MessageId = MessageId(0x3000e4);
/// The header's `D3D11_MESSAGE_ID_GETCRYPTOSESSIONCERTIFICATE_WRONGSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCRYPTOSESSIONCERTIFICATE_WRONGSIZE: MessageId = MessageId(0x3000e5);
/// The header's `D3D11_MESSAGE_ID_GETCRYPTOSESSIONHANDLE_WRONGSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCRYPTOSESSIONHANDLE_WRONGSIZE: MessageId = MessageId(0x3000e6);
/// The header's `D3D11_MESSAGE_ID_NEGOTIATECRPYTOSESSIONKEYEXCHANGE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NEGOTIATECRPYTOSESSIONKEYEXCHANGE_NULLPARAM: MessageId =
    MessageId(0x3000e7);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_UNSUPPORTED: MessageId = MessageId(0x3000e8);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_NULLPARAM: MessageId = MessageId(0x3000e9);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_WRONGDEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_WRONGDEVICE: MessageId = MessageId(0x3000ea);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_DST_WRONGDEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_DST_WRONGDEVICE: MessageId = MessageId(0x3000eb);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_FORMAT_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_FORMAT_MISMATCH: MessageId = MessageId(0x3000ec);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_SIZE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_SIZE_MISMATCH: MessageId = MessageId(0x3000ed);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_MULTISAMPLED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_MULTISAMPLED: MessageId = MessageId(0x3000ee);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_DST_NOT_STAGING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_DST_NOT_STAGING: MessageId = MessageId(0x3000ef);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_MAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_MAPPED: MessageId = MessageId(0x3000f0);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_DST_MAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_DST_MAPPED: MessageId = MessageId(0x3000f1);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_OFFERED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_OFFERED: MessageId = MessageId(0x3000f2);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_DST_OFFERED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_DST_OFFERED: MessageId = MessageId(0x3000f3);
/// The header's `D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_CONTENT_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENCRYPTIONBLT_SRC_CONTENT_UNDEFINED: MessageId = MessageId(0x3000f4);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_UNSUPPORTED: MessageId = MessageId(0x3000f5);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_NULLPARAM: MessageId = MessageId(0x3000f6);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_WRONGDEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_WRONGDEVICE: MessageId = MessageId(0x3000f7);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_WRONGDEVICE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_WRONGDEVICE: MessageId = MessageId(0x3000f8);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_FORMAT_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_FORMAT_MISMATCH: MessageId = MessageId(0x3000f9);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_SIZE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_SIZE_MISMATCH: MessageId = MessageId(0x3000fa);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_MULTISAMPLED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_MULTISAMPLED: MessageId = MessageId(0x3000fb);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_NOT_STAGING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_NOT_STAGING: MessageId = MessageId(0x3000fc);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_NOT_RENDER_TARGET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_NOT_RENDER_TARGET: MessageId = MessageId(0x3000fd);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_MAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_MAPPED: MessageId = MessageId(0x3000fe);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_MAPPED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_MAPPED: MessageId = MessageId(0x3000ff);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_OFFERED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_OFFERED: MessageId = MessageId(0x300100);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_OFFERED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_DST_OFFERED: MessageId = MessageId(0x300101);
/// The header's `D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_CONTENT_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECRYPTIONBLT_SRC_CONTENT_UNDEFINED: MessageId = MessageId(0x300102);
/// The header's `D3D11_MESSAGE_ID_STARTSESSIONKEYREFRESH_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_STARTSESSIONKEYREFRESH_NULLPARAM: MessageId = MessageId(0x300103);
/// The header's `D3D11_MESSAGE_ID_STARTSESSIONKEYREFRESH_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_STARTSESSIONKEYREFRESH_INVALIDSIZE: MessageId = MessageId(0x300104);
/// The header's `D3D11_MESSAGE_ID_FINISHSESSIONKEYREFRESH_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_FINISHSESSIONKEYREFRESH_NULLPARAM: MessageId = MessageId(0x300105);
/// The header's `D3D11_MESSAGE_ID_GETENCRYPTIONBLTKEY_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETENCRYPTIONBLTKEY_NULLPARAM: MessageId = MessageId(0x300106);
/// The header's `D3D11_MESSAGE_ID_GETENCRYPTIONBLTKEY_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETENCRYPTIONBLTKEY_INVALIDSIZE: MessageId = MessageId(0x300107);
/// The header's `D3D11_MESSAGE_ID_GETCONTENTPROTECTIONCAPS_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCONTENTPROTECTIONCAPS_NULLPARAM: MessageId = MessageId(0x300108);
/// The header's `D3D11_MESSAGE_ID_CHECKCRYPTOKEYEXCHANGE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKCRYPTOKEYEXCHANGE_NULLPARAM: MessageId = MessageId(0x300109);
/// The header's `D3D11_MESSAGE_ID_CHECKCRYPTOKEYEXCHANGE_INVALIDINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKCRYPTOKEYEXCHANGE_INVALIDINDEX: MessageId = MessageId(0x30010a);
/// The header's `D3D11_MESSAGE_ID_CREATEAUTHENTICATEDCHANNEL_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEAUTHENTICATEDCHANNEL_NULLPARAM: MessageId = MessageId(0x30010b);
/// The header's `D3D11_MESSAGE_ID_CREATEAUTHENTICATEDCHANNEL_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEAUTHENTICATEDCHANNEL_UNSUPPORTED: MessageId = MessageId(0x30010c);
/// The header's `D3D11_MESSAGE_ID_CREATEAUTHENTICATEDCHANNEL_INVALIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEAUTHENTICATEDCHANNEL_INVALIDTYPE: MessageId = MessageId(0x30010d);
/// The header's `D3D11_MESSAGE_ID_CREATEAUTHENTICATEDCHANNEL_OUTOFMEMORY_RETURN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEAUTHENTICATEDCHANNEL_OUTOFMEMORY_RETURN: MessageId =
    MessageId(0x30010e);
/// The header's `D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATESIZE_INVALIDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATESIZE_INVALIDCHANNEL: MessageId =
    MessageId(0x30010f);
/// The header's `D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATESIZE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATESIZE_NULLPARAM: MessageId =
    MessageId(0x300110);
/// The header's `D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATE_INVALIDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATE_INVALIDCHANNEL: MessageId =
    MessageId(0x300111);
/// The header's `D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATE_NULLPARAM: MessageId =
    MessageId(0x300112);
/// The header's `D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATE_WRONGSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETAUTHENTICATEDCHANNELCERTIFICATE_WRONGSIZE: MessageId =
    MessageId(0x300113);
/// The header's `D3D11_MESSAGE_ID_NEGOTIATEAUTHENTICATEDCHANNELKEYEXCHANGE_INVALIDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NEGOTIATEAUTHENTICATEDCHANNELKEYEXCHANGE_INVALIDCHANNEL: MessageId =
    MessageId(0x300114);
/// The header's `D3D11_MESSAGE_ID_NEGOTIATEAUTHENTICATEDCHANNELKEYEXCHANGE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NEGOTIATEAUTHENTICATEDCHANNELKEYEXCHANGE_NULLPARAM: MessageId =
    MessageId(0x300115);
/// The header's `D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_NULLPARAM: MessageId = MessageId(0x300116);
/// The header's `D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_WRONGCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_WRONGCHANNEL: MessageId = MessageId(0x300117);
/// The header's `D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_UNSUPPORTEDQUERY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_UNSUPPORTEDQUERY: MessageId =
    MessageId(0x300118);
/// The header's `D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_WRONGSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_WRONGSIZE: MessageId = MessageId(0x300119);
/// The header's `D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_INVALIDPROCESSINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_QUERYAUTHENTICATEDCHANNEL_INVALIDPROCESSINDEX: MessageId =
    MessageId(0x30011a);
/// The header's `D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_NULLPARAM: MessageId = MessageId(0x30011b);
/// The header's `D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_WRONGCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_WRONGCHANNEL: MessageId =
    MessageId(0x30011c);
/// The header's `D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_UNSUPPORTEDCONFIGURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_UNSUPPORTEDCONFIGURE: MessageId =
    MessageId(0x30011d);
/// The header's `D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_WRONGSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_WRONGSIZE: MessageId = MessageId(0x30011e);
/// The header's `D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_INVALIDPROCESSIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CONFIGUREAUTHENTICATEDCHANNEL_INVALIDPROCESSIDTYPE: MessageId =
    MessageId(0x30011f);
/// The header's `D3D11_MESSAGE_ID_VSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT: MessageId =
    MessageId(0x300120);
/// The header's `D3D11_MESSAGE_ID_DSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT: MessageId =
    MessageId(0x300121);
/// The header's `D3D11_MESSAGE_ID_HSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_HSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT: MessageId =
    MessageId(0x300122);
/// The header's `D3D11_MESSAGE_ID_GSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT: MessageId =
    MessageId(0x300123);
/// The header's `D3D11_MESSAGE_ID_PSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_PSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT: MessageId =
    MessageId(0x300124);
/// The header's `D3D11_MESSAGE_ID_CSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CSSETCONSTANTBUFFERS_INVALIDBUFFEROFFSETORCOUNT: MessageId =
    MessageId(0x300125);
/// The header's `D3D11_MESSAGE_ID_NEGOTIATECRPYTOSESSIONKEYEXCHANGE_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NEGOTIATECRPYTOSESSIONKEYEXCHANGE_INVALIDSIZE: MessageId =
    MessageId(0x300126);
/// The header's `D3D11_MESSAGE_ID_NEGOTIATEAUTHENTICATEDCHANNELKEYEXCHANGE_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NEGOTIATEAUTHENTICATEDCHANNELKEYEXCHANGE_INVALIDSIZE: MessageId =
    MessageId(0x300127);
/// The header's `D3D11_MESSAGE_ID_OFFERRESOURCES_INVALIDPRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OFFERRESOURCES_INVALIDPRIORITY: MessageId = MessageId(0x300128);
/// The header's `D3D11_MESSAGE_ID_GETCRYPTOSESSIONHANDLE_OUTOFMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCRYPTOSESSIONHANDLE_OUTOFMEMORY: MessageId = MessageId(0x300129);
/// The header's `D3D11_MESSAGE_ID_ACQUIREHANDLEFORCAPTURE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ACQUIREHANDLEFORCAPTURE_NULLPARAM: MessageId = MessageId(0x30012a);
/// The header's `D3D11_MESSAGE_ID_ACQUIREHANDLEFORCAPTURE_INVALIDTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ACQUIREHANDLEFORCAPTURE_INVALIDTYPE: MessageId = MessageId(0x30012b);
/// The header's `D3D11_MESSAGE_ID_ACQUIREHANDLEFORCAPTURE_INVALIDBIND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ACQUIREHANDLEFORCAPTURE_INVALIDBIND: MessageId = MessageId(0x30012c);
/// The header's `D3D11_MESSAGE_ID_ACQUIREHANDLEFORCAPTURE_INVALIDARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ACQUIREHANDLEFORCAPTURE_INVALIDARRAY: MessageId = MessageId(0x30012d);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMROTATION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMROTATION_NULLPARAM: MessageId =
    MessageId(0x30012e);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMROTATION_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMROTATION_INVALIDSTREAM: MessageId =
    MessageId(0x30012f);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMROTATION_INVALID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMROTATION_INVALID: MessageId = MessageId(0x300130);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMROTATION_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMROTATION_UNSUPPORTED: MessageId =
    MessageId(0x300131);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMROTATION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMROTATION_NULLPARAM: MessageId =
    MessageId(0x300132);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_INVALIDVIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_INVALIDVIEW: MessageId = MessageId(0x300133);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x300134);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_SHADEREXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_SHADEREXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x300135);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEHULLSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEHULLSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x300136);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEHULLSHADER_SHADEREXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEHULLSHADER_SHADEREXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x300137);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x300138);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_SHADEREXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_SHADEREXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x300139);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x30013a);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_SHADEREXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_SHADEREXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x30013b);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId = MessageId(0x30013c);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_SHADEREXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_SHADEREXTENSIONSNOTSUPPORTED: MessageId = MessageId(0x30013d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x30013e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_SHADEREXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_SHADEREXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x30013f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_DOUBLEEXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_DOUBLEEXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x300140);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_SHADEREXTENSIONSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_SHADEREXTENSIONSNOTSUPPORTED: MessageId =
    MessageId(0x300141);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_MINPRECISION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SHADER_LINKAGE_MINPRECISION: MessageId = MessageId(0x300142);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMALPHA_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMALPHA_UNSUPPORTED: MessageId =
    MessageId(0x300143);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPIXELASPECTRATIO_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMPIXELASPECTRATIO_UNSUPPORTED: MessageId =
    MessageId(0x300144);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEVERTEXSHADER_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x300145);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEHULLSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEHULLSHADER_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x300146);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEDOMAINSHADER_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x300147);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADER_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x300148);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEGEOMETRYSHADERWITHSTREAMOUTPUT_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x300149);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATEPIXELSHADER_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x30014a);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_UAVSNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CREATECOMPUTESHADER_UAVSNOTSUPPORTED: MessageId =
    MessageId(0x30014b);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_INVALIDOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_INVALIDOFFSET:
    MessageId = MessageId(0x30014c);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_TOOMANYVIEWS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OMSETRENDERTARGETSANDUNORDEREDACCESSVIEWS_TOOMANYVIEWS:
    MessageId = MessageId(0x30014d);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_NOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_NOTSUPPORTED: MessageId = MessageId(0x30014e);
/// The header's `D3D11_MESSAGE_ID_SWAPDEVICECONTEXTSTATE_NOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_SWAPDEVICECONTEXTSTATE_NOTSUPPORTED: MessageId = MessageId(0x30014f);
/// The header's `D3D11_MESSAGE_ID_UPDATESUBRESOURCE_PREFERUPDATESUBRESOURCE1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UPDATESUBRESOURCE_PREFERUPDATESUBRESOURCE1: MessageId =
    MessageId(0x300150);
/// The header's `D3D11_MESSAGE_ID_GETDC_INACCESSIBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDC_INACCESSIBLE: MessageId = MessageId(0x300151);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_INVALIDRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_INVALIDRECT: MessageId = MessageId(0x300152);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_SAMPLE_MASK_IGNORED_ON_FL9`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_SAMPLE_MASK_IGNORED_ON_FL9: MessageId = MessageId(0x300153);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE1_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE1_NOT_SUPPORTED: MessageId =
    MessageId(0x300154);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE_BY_NAME_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE_BY_NAME_NOT_SUPPORTED: MessageId =
    MessageId(0x300155);
/// The header's `D3D11_MESSAGE_ID_ENQUEUESETEVENT_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_ENQUEUESETEVENT_NOT_SUPPORTED: MessageId = MessageId(0x300156);
/// The header's `D3D11_MESSAGE_ID_OFFERRELEASE_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OFFERRELEASE_NOT_SUPPORTED: MessageId = MessageId(0x300157);
/// The header's `D3D11_MESSAGE_ID_OFFERRESOURCES_INACCESSIBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OFFERRESOURCES_INACCESSIBLE: MessageId = MessageId(0x300158);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDMSAA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSORINPUTVIEW_INVALIDMSAA: MessageId =
    MessageId(0x300159);
/// The header's `D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDMSAA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEVIDEOPROCESSOROUTPUTVIEW_INVALIDMSAA: MessageId =
    MessageId(0x30015a);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_INVALIDSOURCERECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_INVALIDSOURCERECT: MessageId = MessageId(0x30015b);
/// The header's `D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_EMPTYRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_CLEARVIEW_EMPTYRECT: MessageId = MessageId(0x30015c);
/// The header's `D3D11_MESSAGE_ID_UPDATESUBRESOURCE_EMPTYDESTBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UPDATESUBRESOURCE_EMPTYDESTBOX: MessageId = MessageId(0x30015d);
/// The header's `D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_EMPTYSOURCEBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYSUBRESOURCEREGION_EMPTYSOURCEBOX: MessageId = MessageId(0x30015e);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_OM_RENDER_TARGET_DOES_NOT_SUPPORT_LOGIC_OPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_OM_RENDER_TARGET_DOES_NOT_SUPPORT_LOGIC_OPS: MessageId =
    MessageId(0x30015f);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_DEPTHSTENCILVIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_DEPTHSTENCILVIEW_NOT_SET: MessageId = MessageId(0x300160);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RENDERTARGETVIEW_NOT_SET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RENDERTARGETVIEW_NOT_SET: MessageId = MessageId(0x300161);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_RENDERTARGETVIEW_NOT_SET_DUE_TO_FLIP_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_RENDERTARGETVIEW_NOT_SET_DUE_TO_FLIP_PRESENT: MessageId =
    MessageId(0x300162);
/// The header's `D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_NOT_SET_DUE_TO_FLIP_PRESENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_UNORDEREDACCESSVIEW_NOT_SET_DUE_TO_FLIP_PRESENT: MessageId =
    MessageId(0x300163);
/// The header's `D3D11_MESSAGE_ID_GETDATAFORNEWHARDWAREKEY_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETDATAFORNEWHARDWAREKEY_NULLPARAM: MessageId = MessageId(0x300164);
/// The header's `D3D11_MESSAGE_ID_CHECKCRYPTOSESSIONSTATUS_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKCRYPTOSESSIONSTATUS_NULLPARAM: MessageId = MessageId(0x300165);
/// The header's `D3D11_MESSAGE_ID_GETCRYPTOSESSIONPRIVATEDATASIZE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCRYPTOSESSIONPRIVATEDATASIZE_NULLPARAM: MessageId =
    MessageId(0x300166);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERCAPS_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERCAPS_NULLPARAM: MessageId = MessageId(0x300167);
/// The header's `D3D11_MESSAGE_ID_GETVIDEODECODERCAPS_ZEROWIDTHHEIGHT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETVIDEODECODERCAPS_ZEROWIDTHHEIGHT: MessageId = MessageId(0x300168);
/// The header's `D3D11_MESSAGE_ID_CHECKVIDEODECODERDOWNSAMPLING_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKVIDEODECODERDOWNSAMPLING_NULLPARAM: MessageId = MessageId(0x300169);
/// The header's `D3D11_MESSAGE_ID_CHECKVIDEODECODERDOWNSAMPLING_INVALIDCOLORSPACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKVIDEODECODERDOWNSAMPLING_INVALIDCOLORSPACE: MessageId =
    MessageId(0x30016a);
/// The header's `D3D11_MESSAGE_ID_CHECKVIDEODECODERDOWNSAMPLING_ZEROWIDTHHEIGHT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKVIDEODECODERDOWNSAMPLING_ZEROWIDTHHEIGHT: MessageId =
    MessageId(0x30016b);
/// The header's `D3D11_MESSAGE_ID_VIDEODECODERENABLEDOWNSAMPLING_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEODECODERENABLEDOWNSAMPLING_NULLPARAM: MessageId =
    MessageId(0x30016c);
/// The header's `D3D11_MESSAGE_ID_VIDEODECODERENABLEDOWNSAMPLING_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEODECODERENABLEDOWNSAMPLING_UNSUPPORTED: MessageId =
    MessageId(0x30016d);
/// The header's `D3D11_MESSAGE_ID_VIDEODECODERUPDATEDOWNSAMPLING_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEODECODERUPDATEDOWNSAMPLING_NULLPARAM: MessageId =
    MessageId(0x30016e);
/// The header's `D3D11_MESSAGE_ID_VIDEODECODERUPDATEDOWNSAMPLING_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEODECODERUPDATEDOWNSAMPLING_UNSUPPORTED: MessageId =
    MessageId(0x30016f);
/// The header's `D3D11_MESSAGE_ID_CHECKVIDEOPROCESSORFORMATCONVERSION_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKVIDEOPROCESSORFORMATCONVERSION_NULLPARAM: MessageId =
    MessageId(0x300170);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCOLORSPACE1_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTCOLORSPACE1_NULLPARAM: MessageId =
    MessageId(0x300171);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTCOLORSPACE1_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTCOLORSPACE1_NULLPARAM: MessageId =
    MessageId(0x300172);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMCOLORSPACE1_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMCOLORSPACE1_NULLPARAM: MessageId =
    MessageId(0x300173);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMCOLORSPACE1_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMCOLORSPACE1_INVALIDSTREAM: MessageId =
    MessageId(0x300174);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMMIRROR_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMMIRROR_NULLPARAM: MessageId = MessageId(0x300175);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMMIRROR_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMMIRROR_INVALIDSTREAM: MessageId =
    MessageId(0x300176);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMMIRROR_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMMIRROR_UNSUPPORTED: MessageId =
    MessageId(0x300177);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMCOLORSPACE1_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMCOLORSPACE1_NULLPARAM: MessageId =
    MessageId(0x300178);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMMIRROR_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMMIRROR_NULLPARAM: MessageId = MessageId(0x300179);
/// The header's `D3D11_MESSAGE_ID_RECOMMENDVIDEODECODERDOWNSAMPLING_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RECOMMENDVIDEODECODERDOWNSAMPLING_NULLPARAM: MessageId =
    MessageId(0x30017a);
/// The header's `D3D11_MESSAGE_ID_RECOMMENDVIDEODECODERDOWNSAMPLING_INVALIDCOLORSPACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RECOMMENDVIDEODECODERDOWNSAMPLING_INVALIDCOLORSPACE: MessageId =
    MessageId(0x30017b);
/// The header's `D3D11_MESSAGE_ID_RECOMMENDVIDEODECODERDOWNSAMPLING_ZEROWIDTHHEIGHT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RECOMMENDVIDEODECODERDOWNSAMPLING_ZEROWIDTHHEIGHT: MessageId =
    MessageId(0x30017c);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTSHADERUSAGE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTSHADERUSAGE_NULLPARAM: MessageId =
    MessageId(0x30017d);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTSHADERUSAGE_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTSHADERUSAGE_NULLPARAM: MessageId =
    MessageId(0x30017e);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_NULLPARAM: MessageId =
    MessageId(0x30017f);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_INVALIDSTREAMCOUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_INVALIDSTREAMCOUNT: MessageId =
    MessageId(0x300180);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_TARGETRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_TARGETRECT: MessageId =
    MessageId(0x300181);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_INVALIDSOURCERECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_INVALIDSOURCERECT: MessageId =
    MessageId(0x300182);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_INVALIDDESTRECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETBEHAVIORHINTS_INVALIDDESTRECT: MessageId =
    MessageId(0x300183);
/// The header's `D3D11_MESSAGE_ID_GETCRYPTOSESSIONPRIVATEDATASIZE_INVALID_KEY_EXCHANGE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETCRYPTOSESSIONPRIVATEDATASIZE_INVALID_KEY_EXCHANGE_TYPE: MessageId =
    MessageId(0x300184);
/// The header's `D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE1_ACCESS_DENIED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_OPEN_SHARED_RESOURCE1_ACCESS_DENIED: MessageId =
    MessageId(0x300185);
/// The header's `D3D11_MESSAGE_ID_D3D11_1_MESSAGES_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_1_MESSAGES_END: MessageId = MessageId(0x300186);
/// The header's `D3D11_MESSAGE_ID_D3D11_2_MESSAGES_START`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_2_MESSAGES_START: MessageId = MessageId(0x300187);
/// The header's `D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEBUFFER_INVALIDUSAGE: MessageId = MessageId(0x300188);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE1D_INVALIDUSAGE: MessageId = MessageId(0x300189);
/// The header's `D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATETEXTURE2D_INVALIDUSAGE: MessageId = MessageId(0x30018a);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_LEVEL9_STEPRATE_NOT_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_LEVEL9_STEPRATE_NOT_1: MessageId = MessageId(0x30018b);
/// The header's `D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_LEVEL9_INSTANCING_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEINPUTLAYOUT_LEVEL9_INSTANCING_NOT_SUPPORTED: MessageId =
    MessageId(0x30018c);
/// The header's `D3D11_MESSAGE_ID_UPDATETILEMAPPINGS_INVALID_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UPDATETILEMAPPINGS_INVALID_PARAMETER: MessageId = MessageId(0x30018d);
/// The header's `D3D11_MESSAGE_ID_COPYTILEMAPPINGS_INVALID_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYTILEMAPPINGS_INVALID_PARAMETER: MessageId = MessageId(0x30018e);
/// The header's `D3D11_MESSAGE_ID_COPYTILES_INVALID_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_COPYTILES_INVALID_PARAMETER: MessageId = MessageId(0x30018f);
/// The header's `D3D11_MESSAGE_ID_UPDATETILES_INVALID_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_UPDATETILES_INVALID_PARAMETER: MessageId = MessageId(0x300190);
/// The header's `D3D11_MESSAGE_ID_RESIZETILEPOOL_INVALID_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESIZETILEPOOL_INVALID_PARAMETER: MessageId = MessageId(0x300191);
/// The header's `D3D11_MESSAGE_ID_TILEDRESOURCEBARRIER_INVALID_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TILEDRESOURCEBARRIER_INVALID_PARAMETER: MessageId = MessageId(0x300192);
/// The header's `D3D11_MESSAGE_ID_NULL_TILE_MAPPING_ACCESS_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NULL_TILE_MAPPING_ACCESS_WARNING: MessageId = MessageId(0x300193);
/// The header's `D3D11_MESSAGE_ID_NULL_TILE_MAPPING_ACCESS_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NULL_TILE_MAPPING_ACCESS_ERROR: MessageId = MessageId(0x300194);
/// The header's `D3D11_MESSAGE_ID_DIRTY_TILE_MAPPING_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DIRTY_TILE_MAPPING_ACCESS: MessageId = MessageId(0x300195);
/// The header's `D3D11_MESSAGE_ID_DUPLICATE_TILE_MAPPINGS_IN_COVERED_AREA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DUPLICATE_TILE_MAPPINGS_IN_COVERED_AREA: MessageId = MessageId(0x300196);
/// The header's `D3D11_MESSAGE_ID_TILE_MAPPINGS_IN_COVERED_AREA_DUPLICATED_OUTSIDE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TILE_MAPPINGS_IN_COVERED_AREA_DUPLICATED_OUTSIDE: MessageId =
    MessageId(0x300197);
/// The header's `D3D11_MESSAGE_ID_TILE_MAPPINGS_SHARED_BETWEEN_INCOMPATIBLE_RESOURCES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TILE_MAPPINGS_SHARED_BETWEEN_INCOMPATIBLE_RESOURCES: MessageId =
    MessageId(0x300198);
/// The header's `D3D11_MESSAGE_ID_TILE_MAPPINGS_SHARED_BETWEEN_INPUT_AND_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TILE_MAPPINGS_SHARED_BETWEEN_INPUT_AND_OUTPUT: MessageId =
    MessageId(0x300199);
/// The header's `D3D11_MESSAGE_ID_CHECKMULTISAMPLEQUALITYLEVELS_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CHECKMULTISAMPLEQUALITYLEVELS_INVALIDFLAGS: MessageId =
    MessageId(0x30019a);
/// The header's `D3D11_MESSAGE_ID_GETRESOURCETILING_NONTILED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_GETRESOURCETILING_NONTILED_RESOURCE: MessageId = MessageId(0x30019b);
/// The header's `D3D11_MESSAGE_ID_RESIZETILEPOOL_SHRINK_WITH_MAPPINGS_STILL_DEFINED_PAST_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_RESIZETILEPOOL_SHRINK_WITH_MAPPINGS_STILL_DEFINED_PAST_END: MessageId =
    MessageId(0x30019c);
/// The header's `D3D11_MESSAGE_ID_NEED_TO_CALL_TILEDRESOURCEBARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NEED_TO_CALL_TILEDRESOURCEBARRIER: MessageId = MessageId(0x30019d);
/// The header's `D3D11_MESSAGE_ID_CREATEDEVICE_INVALIDARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEVICE_INVALIDARGS: MessageId = MessageId(0x30019e);
/// The header's `D3D11_MESSAGE_ID_CREATEDEVICE_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEDEVICE_WARNING: MessageId = MessageId(0x30019f);
/// The header's `D3D11_MESSAGE_ID_CLEARUNORDEREDACCESSVIEWUINT_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARUNORDEREDACCESSVIEWUINT_HAZARD: MessageId = MessageId(0x3001a0);
/// The header's `D3D11_MESSAGE_ID_CLEARUNORDEREDACCESSVIEWFLOAT_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CLEARUNORDEREDACCESSVIEWFLOAT_HAZARD: MessageId = MessageId(0x3001a1);
/// The header's `D3D11_MESSAGE_ID_TILED_RESOURCE_TIER_1_BUFFER_TEXTURE_MISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TILED_RESOURCE_TIER_1_BUFFER_TEXTURE_MISMATCH: MessageId =
    MessageId(0x3001a2);
/// The header's `D3D11_MESSAGE_ID_CREATE_CRYPTOSESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_CRYPTOSESSION: MessageId = MessageId(0x3001a3);
/// The header's `D3D11_MESSAGE_ID_CREATE_AUTHENTICATEDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_AUTHENTICATEDCHANNEL: MessageId = MessageId(0x3001a4);
/// The header's `D3D11_MESSAGE_ID_LIVE_CRYPTOSESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_CRYPTOSESSION: MessageId = MessageId(0x3001a5);
/// The header's `D3D11_MESSAGE_ID_LIVE_AUTHENTICATEDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_AUTHENTICATEDCHANNEL: MessageId = MessageId(0x3001a6);
/// The header's `D3D11_MESSAGE_ID_DESTROY_CRYPTOSESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_CRYPTOSESSION: MessageId = MessageId(0x3001a7);
/// The header's `D3D11_MESSAGE_ID_DESTROY_AUTHENTICATEDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_AUTHENTICATEDCHANNEL: MessageId = MessageId(0x3001a8);
/// The header's `D3D11_MESSAGE_ID_D3D11_2_MESSAGES_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_2_MESSAGES_END: MessageId = MessageId(0x3001a9);
/// The header's `D3D11_MESSAGE_ID_D3D11_3_MESSAGES_START`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_3_MESSAGES_START: MessageId = MessageId(0x3001aa);
/// The header's `D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALID_CONSERVATIVERASTERMODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERASTERIZERSTATE_INVALID_CONSERVATIVERASTERMODE: MessageId =
    MessageId(0x3001ab);
/// The header's `D3D11_MESSAGE_ID_DEVICE_DRAW_INVALID_SYSTEMVALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_DRAW_INVALID_SYSTEMVALUE: MessageId = MessageId(0x3001ac);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_INVALIDCONTEXTTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_INVALIDCONTEXTTYPE: MessageId =
    MessageId(0x3001ad);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_DECODENOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_DECODENOTSUPPORTED: MessageId =
    MessageId(0x3001ae);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_ENCODENOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_ENCODENOTSUPPORTED: MessageId =
    MessageId(0x3001af);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDPLANEINDEX: MessageId =
    MessageId(0x3001b0);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDVIDEOPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_INVALIDVIDEOPLANEINDEX: MessageId =
    MessageId(0x3001b1);
/// The header's `D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_AMBIGUOUSVIDEOPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATESHADERRESOURCEVIEW_AMBIGUOUSVIDEOPLANEINDEX: MessageId =
    MessageId(0x3001b2);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDPLANEINDEX: MessageId =
    MessageId(0x3001b3);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDVIDEOPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_INVALIDVIDEOPLANEINDEX: MessageId =
    MessageId(0x3001b4);
/// The header's `D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_AMBIGUOUSVIDEOPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATERENDERTARGETVIEW_AMBIGUOUSVIDEOPLANEINDEX: MessageId =
    MessageId(0x3001b5);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDPLANEINDEX: MessageId =
    MessageId(0x3001b6);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDVIDEOPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_INVALIDVIDEOPLANEINDEX: MessageId =
    MessageId(0x3001b7);
/// The header's `D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_AMBIGUOUSVIDEOPLANEINDEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEUNORDEREDACCESSVIEW_AMBIGUOUSVIDEOPLANEINDEX: MessageId =
    MessageId(0x3001b8);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_INVALIDSCANDATAOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_INVALIDSCANDATAOFFSET: MessageId = MessageId(0x3001b9);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_NOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_NOTSUPPORTED: MessageId = MessageId(0x3001ba);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_DIMENSIONSTOOLARGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_DIMENSIONSTOOLARGE: MessageId = MessageId(0x3001bb);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_INVALIDCOMPONENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_INVALIDCOMPONENTS: MessageId = MessageId(0x3001bc);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_DESTINATIONNOT2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_DESTINATIONNOT2D: MessageId = MessageId(0x3001bd);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_TILEDRESOURCESUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_TILEDRESOURCESUNSUPPORTED: MessageId = MessageId(0x3001be);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_GUARDRECTSUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_GUARDRECTSUNSUPPORTED: MessageId = MessageId(0x3001bf);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_FORMATUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_FORMATUNSUPPORTED: MessageId = MessageId(0x3001c0);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_INVALIDSUBRESOURCE: MessageId = MessageId(0x3001c1);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_INVALIDMIPLEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_INVALIDMIPLEVEL: MessageId = MessageId(0x3001c2);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_EMPTYDESTBOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_EMPTYDESTBOX: MessageId = MessageId(0x3001c3);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_DESTBOXNOT2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_DESTBOXNOT2D: MessageId = MessageId(0x3001c4);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_DESTBOXNOTSUB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_DESTBOXNOTSUB: MessageId = MessageId(0x3001c5);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_DESTBOXESINTERSECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_DESTBOXESINTERSECT: MessageId = MessageId(0x3001c6);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_XSUBSAMPLEMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_XSUBSAMPLEMISMATCH: MessageId = MessageId(0x3001c7);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_YSUBSAMPLEMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_YSUBSAMPLEMISMATCH: MessageId = MessageId(0x3001c8);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_XSUBSAMPLEODD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_XSUBSAMPLEODD: MessageId = MessageId(0x3001c9);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_YSUBSAMPLEODD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_YSUBSAMPLEODD: MessageId = MessageId(0x3001ca);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_OUTPUTDIMENSIONSTOOLARGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_OUTPUTDIMENSIONSTOOLARGE: MessageId = MessageId(0x3001cb);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_NONPOW2SCALEUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_NONPOW2SCALEUNSUPPORTED: MessageId = MessageId(0x3001cc);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_FRACTIONALDOWNSCALETOLARGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_FRACTIONALDOWNSCALETOLARGE: MessageId = MessageId(0x3001cd);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_CHROMASIZEMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_CHROMASIZEMISMATCH: MessageId = MessageId(0x3001ce);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_LUMACHROMASIZEMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_LUMACHROMASIZEMISMATCH: MessageId = MessageId(0x3001cf);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_INVALIDNUMDESTINATIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_INVALIDNUMDESTINATIONS: MessageId = MessageId(0x3001d0);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_SUBBOXUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_SUBBOXUNSUPPORTED: MessageId = MessageId(0x3001d1);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_1DESTUNSUPPORTEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_1DESTUNSUPPORTEDFORMAT: MessageId = MessageId(0x3001d2);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_3DESTUNSUPPORTEDFORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_3DESTUNSUPPORTEDFORMAT: MessageId = MessageId(0x3001d3);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_SCALEUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_SCALEUNSUPPORTED: MessageId = MessageId(0x3001d4);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_INVALIDSOURCESIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_INVALIDSOURCESIZE: MessageId = MessageId(0x3001d5);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_INVALIDCOPYFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_INVALIDCOPYFLAGS: MessageId = MessageId(0x3001d6);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_HAZARD: MessageId = MessageId(0x3001d7);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_UNSUPPORTEDSRCBUFFERUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_UNSUPPORTEDSRCBUFFERUSAGE: MessageId = MessageId(0x3001d8);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_UNSUPPORTEDSRCBUFFERMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_UNSUPPORTEDSRCBUFFERMISCFLAGS: MessageId =
    MessageId(0x3001d9);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_UNSUPPORTEDDSTTEXTUREUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_UNSUPPORTEDDSTTEXTUREUSAGE: MessageId = MessageId(0x3001da);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_BACKBUFFERNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_BACKBUFFERNOTSUPPORTED: MessageId = MessageId(0x3001db);
/// The header's `D3D11_MESSAGE_ID_JPEGDECODE_UNSUPPRTEDCOPYFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGDECODE_UNSUPPRTEDCOPYFLAGS: MessageId = MessageId(0x3001dc);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_NOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_NOTSUPPORTED: MessageId = MessageId(0x3001dd);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_INVALIDSCANDATAOFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_INVALIDSCANDATAOFFSET: MessageId = MessageId(0x3001de);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_INVALIDCOMPONENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_INVALIDCOMPONENTS: MessageId = MessageId(0x3001df);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_SOURCENOT2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_SOURCENOT2D: MessageId = MessageId(0x3001e0);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_TILEDRESOURCESUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_TILEDRESOURCESUNSUPPORTED: MessageId = MessageId(0x3001e1);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_GUARDRECTSUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_GUARDRECTSUNSUPPORTED: MessageId = MessageId(0x3001e2);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_XSUBSAMPLEMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_XSUBSAMPLEMISMATCH: MessageId = MessageId(0x3001e3);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_YSUBSAMPLEMISMATCH`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_YSUBSAMPLEMISMATCH: MessageId = MessageId(0x3001e4);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_FORMATUNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_FORMATUNSUPPORTED: MessageId = MessageId(0x3001e5);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_INVALIDSUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_INVALIDSUBRESOURCE: MessageId = MessageId(0x3001e6);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_INVALIDMIPLEVEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_INVALIDMIPLEVEL: MessageId = MessageId(0x3001e7);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_DIMENSIONSTOOLARGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_DIMENSIONSTOOLARGE: MessageId = MessageId(0x3001e8);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_HAZARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_HAZARD: MessageId = MessageId(0x3001e9);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_UNSUPPORTEDDSTBUFFERUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_UNSUPPORTEDDSTBUFFERUSAGE: MessageId = MessageId(0x3001ea);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_UNSUPPORTEDDSTBUFFERMISCFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_UNSUPPORTEDDSTBUFFERMISCFLAGS: MessageId =
    MessageId(0x3001eb);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_UNSUPPORTEDSRCTEXTUREUSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_UNSUPPORTEDSRCTEXTUREUSAGE: MessageId = MessageId(0x3001ec);
/// The header's `D3D11_MESSAGE_ID_JPEGENCODE_BACKBUFFERNOTSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_JPEGENCODE_BACKBUFFERNOTSUPPORTED: MessageId = MessageId(0x3001ed);
/// The header's `D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_UNSUPPORTEDCONTEXTTTYPEFORQUERY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEQUERYORPREDICATE_UNSUPPORTEDCONTEXTTTYPEFORQUERY: MessageId =
    MessageId(0x3001ee);
/// The header's `D3D11_MESSAGE_ID_FLUSH1_INVALIDCONTEXTTYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_FLUSH1_INVALIDCONTEXTTYPE: MessageId = MessageId(0x3001ef);
/// The header's `D3D11_MESSAGE_ID_DEVICE_SETHARDWAREPROTECTION_INVALIDCONTEXT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DEVICE_SETHARDWAREPROTECTION_INVALIDCONTEXT: MessageId =
    MessageId(0x3001f0);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTHDRMETADATA_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTHDRMETADATA_NULLPARAM: MessageId =
    MessageId(0x3001f1);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTHDRMETADATA_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETOUTPUTHDRMETADATA_INVALIDSIZE: MessageId =
    MessageId(0x3001f2);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTHDRMETADATA_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTHDRMETADATA_NULLPARAM: MessageId =
    MessageId(0x3001f3);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTHDRMETADATA_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETOUTPUTHDRMETADATA_INVALIDSIZE: MessageId =
    MessageId(0x3001f4);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMHDRMETADATA_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMHDRMETADATA_NULLPARAM: MessageId =
    MessageId(0x3001f5);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMHDRMETADATA_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMHDRMETADATA_INVALIDSTREAM: MessageId =
    MessageId(0x3001f6);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMHDRMETADATA_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORSETSTREAMHDRMETADATA_INVALIDSIZE: MessageId =
    MessageId(0x3001f7);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMHDRMETADATA_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMHDRMETADATA_NULLPARAM: MessageId =
    MessageId(0x3001f8);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMHDRMETADATA_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMHDRMETADATA_INVALIDSTREAM: MessageId =
    MessageId(0x3001f9);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMHDRMETADATA_INVALIDSIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMHDRMETADATA_INVALIDSIZE: MessageId =
    MessageId(0x3001fa);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMFRAMEFORMAT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMFRAMEFORMAT_INVALIDSTREAM: MessageId =
    MessageId(0x3001fb);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMCOLORSPACE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMCOLORSPACE_INVALIDSTREAM: MessageId =
    MessageId(0x3001fc);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMOUTPUTRATE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMOUTPUTRATE_INVALIDSTREAM: MessageId =
    MessageId(0x3001fd);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMSOURCERECT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMSOURCERECT_INVALIDSTREAM: MessageId =
    MessageId(0x3001fe);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMDESTRECT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMDESTRECT_INVALIDSTREAM: MessageId =
    MessageId(0x3001ff);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMALPHA_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMALPHA_INVALIDSTREAM: MessageId =
    MessageId(0x300200);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMPALETTE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMPALETTE_INVALIDSTREAM: MessageId =
    MessageId(0x300201);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMPIXELASPECTRATIO_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMPIXELASPECTRATIO_INVALIDSTREAM: MessageId =
    MessageId(0x300202);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMLUMAKEY_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMLUMAKEY_INVALIDSTREAM: MessageId =
    MessageId(0x300203);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMSTEREOFORMAT_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMSTEREOFORMAT_INVALIDSTREAM: MessageId =
    MessageId(0x300204);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMAUTOPROCESSINGMODE_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMAUTOPROCESSINGMODE_INVALIDSTREAM: MessageId =
    MessageId(0x300205);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMFILTER_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMFILTER_INVALIDSTREAM: MessageId =
    MessageId(0x300206);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMROTATION_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMROTATION_INVALIDSTREAM: MessageId =
    MessageId(0x300207);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMCOLORSPACE1_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMCOLORSPACE1_INVALIDSTREAM: MessageId =
    MessageId(0x300208);
/// The header's `D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMMIRROR_INVALIDSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_VIDEOPROCESSORGETSTREAMMIRROR_INVALIDSTREAM: MessageId =
    MessageId(0x300209);
/// The header's `D3D11_MESSAGE_ID_CREATE_FENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_FENCE: MessageId = MessageId(0x30020a);
/// The header's `D3D11_MESSAGE_ID_LIVE_FENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_FENCE: MessageId = MessageId(0x30020b);
/// The header's `D3D11_MESSAGE_ID_DESTROY_FENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_FENCE: MessageId = MessageId(0x30020c);
/// The header's `D3D11_MESSAGE_ID_CREATE_SYNCHRONIZEDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_SYNCHRONIZEDCHANNEL: MessageId = MessageId(0x30020d);
/// The header's `D3D11_MESSAGE_ID_LIVE_SYNCHRONIZEDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_SYNCHRONIZEDCHANNEL: MessageId = MessageId(0x30020e);
/// The header's `D3D11_MESSAGE_ID_DESTROY_SYNCHRONIZEDCHANNEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_SYNCHRONIZEDCHANNEL: MessageId = MessageId(0x30020f);
/// The header's `D3D11_MESSAGE_ID_CREATEFENCE_INVALIDFLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATEFENCE_INVALIDFLAGS: MessageId = MessageId(0x300210);
/// The header's `D3D11_MESSAGE_ID_D3D11_3_MESSAGES_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_3_MESSAGES_END: MessageId = MessageId(0x300211);
/// The header's `D3D11_MESSAGE_ID_D3D11_5_MESSAGES_START`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_5_MESSAGES_START: MessageId = MessageId(0x300212);
/// The header's `D3D11_MESSAGE_ID_NEGOTIATECRYPTOSESSIONKEYEXCHANGEMT_INVALIDKEYEXCHANGETYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NEGOTIATECRYPTOSESSIONKEYEXCHANGEMT_INVALIDKEYEXCHANGETYPE: MessageId =
    MessageId(0x300213);
/// The header's `D3D11_MESSAGE_ID_NEGOTIATECRYPTOSESSIONKEYEXCHANGEMT_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NEGOTIATECRYPTOSESSIONKEYEXCHANGEMT_NOT_SUPPORTED: MessageId =
    MessageId(0x300214);
/// The header's `D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_COMPONENT_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_COMPONENT_COUNT: MessageId =
    MessageId(0x300215);
/// The header's `D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_COMPONENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_COMPONENT: MessageId =
    MessageId(0x300216);
/// The header's `D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_BUFFER_SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_BUFFER_SIZE: MessageId =
    MessageId(0x300217);
/// The header's `D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_BUFFER_USAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_BUFFER_USAGE: MessageId =
    MessageId(0x300218);
/// The header's `D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_BUFFER_MISC_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_BUFFER_MISC_FLAGS: MessageId =
    MessageId(0x300219);
/// The header's `D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_BUFFER_OFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DECODERBEGINFRAME_INVALID_HISTOGRAM_BUFFER_OFFSET: MessageId =
    MessageId(0x30021a);
/// The header's `D3D11_MESSAGE_ID_CREATE_TRACKEDWORKLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_TRACKEDWORKLOAD: MessageId = MessageId(0x30021b);
/// The header's `D3D11_MESSAGE_ID_LIVE_TRACKEDWORKLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_LIVE_TRACKEDWORKLOAD: MessageId = MessageId(0x30021c);
/// The header's `D3D11_MESSAGE_ID_DESTROY_TRACKEDWORKLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_DESTROY_TRACKEDWORKLOAD: MessageId = MessageId(0x30021d);
/// The header's `D3D11_MESSAGE_ID_CREATE_TRACKED_WORKLOAD_NULLPARAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_TRACKED_WORKLOAD_NULLPARAM: MessageId = MessageId(0x30021e);
/// The header's `D3D11_MESSAGE_ID_CREATE_TRACKED_WORKLOAD_INVALID_MAX_INSTANCES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_TRACKED_WORKLOAD_INVALID_MAX_INSTANCES: MessageId =
    MessageId(0x30021f);
/// The header's `D3D11_MESSAGE_ID_CREATE_TRACKED_WORKLOAD_INVALID_DEADLINE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_TRACKED_WORKLOAD_INVALID_DEADLINE_TYPE: MessageId =
    MessageId(0x300220);
/// The header's `D3D11_MESSAGE_ID_CREATE_TRACKED_WORKLOAD_INVALID_ENGINE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CREATE_TRACKED_WORKLOAD_INVALID_ENGINE_TYPE: MessageId =
    MessageId(0x300221);
/// The header's `D3D11_MESSAGE_ID_MULTIPLE_TRACKED_WORKLOADS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_MULTIPLE_TRACKED_WORKLOADS: MessageId = MessageId(0x300222);
/// The header's `D3D11_MESSAGE_ID_MULTIPLE_TRACKED_WORKLOAD_PAIRS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_MULTIPLE_TRACKED_WORKLOAD_PAIRS: MessageId = MessageId(0x300223);
/// The header's `D3D11_MESSAGE_ID_INCOMPLETE_TRACKED_WORKLOAD_PAIR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_INCOMPLETE_TRACKED_WORKLOAD_PAIR: MessageId = MessageId(0x300224);
/// The header's `D3D11_MESSAGE_ID_OUT_OF_ORDER_TRACKED_WORKLOAD_PAIR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_OUT_OF_ORDER_TRACKED_WORKLOAD_PAIR: MessageId = MessageId(0x300225);
/// The header's `D3D11_MESSAGE_ID_CANNOT_ADD_TRACKED_WORKLOAD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_CANNOT_ADD_TRACKED_WORKLOAD: MessageId = MessageId(0x300226);
/// The header's `D3D11_MESSAGE_ID_TRACKED_WORKLOAD_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TRACKED_WORKLOAD_NOT_SUPPORTED: MessageId = MessageId(0x300227);
/// The header's `D3D11_MESSAGE_ID_TRACKED_WORKLOAD_ENGINE_TYPE_NOT_FOUND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TRACKED_WORKLOAD_ENGINE_TYPE_NOT_FOUND: MessageId = MessageId(0x300228);
/// The header's `D3D11_MESSAGE_ID_NO_TRACKED_WORKLOAD_SLOT_AVAILABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_NO_TRACKED_WORKLOAD_SLOT_AVAILABLE: MessageId = MessageId(0x300229);
/// The header's `D3D11_MESSAGE_ID_END_TRACKED_WORKLOAD_INVALID_ARG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_END_TRACKED_WORKLOAD_INVALID_ARG: MessageId = MessageId(0x30022a);
/// The header's `D3D11_MESSAGE_ID_TRACKED_WORKLOAD_DISJOINT_FAILURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_TRACKED_WORKLOAD_DISJOINT_FAILURE: MessageId = MessageId(0x30022b);
/// The header's `D3D11_MESSAGE_ID_D3D11_5_MESSAGES_END`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_id).
pub const D3D11_MESSAGE_ID_D3D11_5_MESSAGES_END: MessageId = MessageId(0x30022c);

/// The header's `D3D11_MESSAGE_SEVERITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_severity).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MessageSeverity(pub i32);

/// The header's `D3D11_MESSAGE_SEVERITY_CORRUPTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_severity).
pub const D3D11_MESSAGE_SEVERITY_CORRUPTION: MessageSeverity = MessageSeverity(0);
/// The header's `D3D11_MESSAGE_SEVERITY_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_severity).
pub const D3D11_MESSAGE_SEVERITY_ERROR: MessageSeverity = MessageSeverity(1);
/// The header's `D3D11_MESSAGE_SEVERITY_WARNING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_severity).
pub const D3D11_MESSAGE_SEVERITY_WARNING: MessageSeverity = MessageSeverity(2);
/// The header's `D3D11_MESSAGE_SEVERITY_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_severity).
pub const D3D11_MESSAGE_SEVERITY_INFO: MessageSeverity = MessageSeverity(3);
/// The header's `D3D11_MESSAGE_SEVERITY_MESSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_message_severity).
pub const D3D11_MESSAGE_SEVERITY_MESSAGE: MessageSeverity = MessageSeverity(4);

/// The header's `D3D11_QUERY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct QueryKind(pub i32);

/// The header's `D3D11_QUERY_EVENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_EVENT: QueryKind = QueryKind(0);
/// The header's `D3D11_QUERY_OCCLUSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_OCCLUSION: QueryKind = QueryKind(1);
/// The header's `D3D11_QUERY_TIMESTAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_TIMESTAMP: QueryKind = QueryKind(2);
/// The header's `D3D11_QUERY_TIMESTAMP_DISJOINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_TIMESTAMP_DISJOINT: QueryKind = QueryKind(3);
/// The header's `D3D11_QUERY_PIPELINE_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_PIPELINE_STATISTICS: QueryKind = QueryKind(4);
/// The header's `D3D11_QUERY_OCCLUSION_PREDICATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_OCCLUSION_PREDICATE: QueryKind = QueryKind(5);
/// The header's `D3D11_QUERY_SO_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_STATISTICS: QueryKind = QueryKind(6);
/// The header's `D3D11_QUERY_SO_OVERFLOW_PREDICATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_OVERFLOW_PREDICATE: QueryKind = QueryKind(7);
/// The header's `D3D11_QUERY_SO_STATISTICS_STREAM0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_STATISTICS_STREAM0: QueryKind = QueryKind(8);
/// The header's `D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM0: QueryKind = QueryKind(9);
/// The header's `D3D11_QUERY_SO_STATISTICS_STREAM1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_STATISTICS_STREAM1: QueryKind = QueryKind(10);
/// The header's `D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM1: QueryKind = QueryKind(11);
/// The header's `D3D11_QUERY_SO_STATISTICS_STREAM2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_STATISTICS_STREAM2: QueryKind = QueryKind(12);
/// The header's `D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM2: QueryKind = QueryKind(13);
/// The header's `D3D11_QUERY_SO_STATISTICS_STREAM3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_STATISTICS_STREAM3: QueryKind = QueryKind(14);
/// The header's `D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query).
pub const D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM3: QueryKind = QueryKind(15);

/// The header's `D3D11_QUERY_MISC_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query_misc_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct QueryMiscFlag(pub i32);

/// The header's `D3D11_QUERY_MISC_PREDICATEHINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query_misc_flag).
pub const D3D11_QUERY_MISC_PREDICATEHINT: QueryMiscFlag = QueryMiscFlag(1);

/// The header's `D3D11_RAISE_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_raise_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RaiseFlag(pub i32);

/// The header's `D3D11_RAISE_FLAG_DRIVER_INTERNAL_ERROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_raise_flag).
pub const D3D11_RAISE_FLAG_DRIVER_INTERNAL_ERROR: RaiseFlag = RaiseFlag(1);

/// The header's `D3D11_RESOURCE_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceDimension(pub i32);

/// The header's `D3D11_RESOURCE_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_dimension).
pub const D3D11_RESOURCE_DIMENSION_UNKNOWN: ResourceDimension = ResourceDimension(0);
/// The header's `D3D11_RESOURCE_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_dimension).
pub const D3D11_RESOURCE_DIMENSION_BUFFER: ResourceDimension = ResourceDimension(1);
/// The header's `D3D11_RESOURCE_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_dimension).
pub const D3D11_RESOURCE_DIMENSION_TEXTURE1D: ResourceDimension = ResourceDimension(2);
/// The header's `D3D11_RESOURCE_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_dimension).
pub const D3D11_RESOURCE_DIMENSION_TEXTURE2D: ResourceDimension = ResourceDimension(3);
/// The header's `D3D11_RESOURCE_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_dimension).
pub const D3D11_RESOURCE_DIMENSION_TEXTURE3D: ResourceDimension = ResourceDimension(4);

/// The header's `D3D11_RESOURCE_MISC_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceMiscFlag(pub i32);

/// The header's `D3D11_RESOURCE_MISC_GENERATE_MIPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_GENERATE_MIPS: ResourceMiscFlag = ResourceMiscFlag(1);
/// The header's `D3D11_RESOURCE_MISC_SHARED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_SHARED: ResourceMiscFlag = ResourceMiscFlag(2);
/// The header's `D3D11_RESOURCE_MISC_TEXTURECUBE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_TEXTURECUBE: ResourceMiscFlag = ResourceMiscFlag(4);
/// The header's `D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS: ResourceMiscFlag = ResourceMiscFlag(16);
/// The header's `D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS: ResourceMiscFlag = ResourceMiscFlag(32);
/// The header's `D3D11_RESOURCE_MISC_BUFFER_STRUCTURED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_BUFFER_STRUCTURED: ResourceMiscFlag = ResourceMiscFlag(64);
/// The header's `D3D11_RESOURCE_MISC_RESOURCE_CLAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_RESOURCE_CLAMP: ResourceMiscFlag = ResourceMiscFlag(128);
/// The header's `D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX: ResourceMiscFlag = ResourceMiscFlag(0x100);
/// The header's `D3D11_RESOURCE_MISC_GDI_COMPATIBLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_GDI_COMPATIBLE: ResourceMiscFlag = ResourceMiscFlag(0x200);
/// The header's `D3D11_RESOURCE_MISC_SHARED_NTHANDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_SHARED_NTHANDLE: ResourceMiscFlag = ResourceMiscFlag(0x800);
/// The header's `D3D11_RESOURCE_MISC_RESTRICTED_CONTENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_RESTRICTED_CONTENT: ResourceMiscFlag = ResourceMiscFlag(0x1000);
/// The header's `D3D11_RESOURCE_MISC_RESTRICT_SHARED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_RESTRICT_SHARED_RESOURCE: ResourceMiscFlag = ResourceMiscFlag(0x2000);
/// The header's `D3D11_RESOURCE_MISC_RESTRICT_SHARED_RESOURCE_DRIVER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_RESTRICT_SHARED_RESOURCE_DRIVER: ResourceMiscFlag =
    ResourceMiscFlag(0x4000);
/// The header's `D3D11_RESOURCE_MISC_GUARDED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_GUARDED: ResourceMiscFlag = ResourceMiscFlag(0x8000);
/// The header's `D3D11_RESOURCE_MISC_TILE_POOL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_TILE_POOL: ResourceMiscFlag = ResourceMiscFlag(0x20000);
/// The header's `D3D11_RESOURCE_MISC_TILED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_TILED: ResourceMiscFlag = ResourceMiscFlag(0x40000);
/// The header's `D3D11_RESOURCE_MISC_HW_PROTECTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_HW_PROTECTED: ResourceMiscFlag = ResourceMiscFlag(0x80000);
/// The header's `D3D11_RESOURCE_MISC_SHARED_DISPLAYABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_SHARED_DISPLAYABLE: ResourceMiscFlag = ResourceMiscFlag(0x100000);
/// The header's `D3D11_RESOURCE_MISC_SHARED_EXCLUSIVE_WRITER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag).
pub const D3D11_RESOURCE_MISC_SHARED_EXCLUSIVE_WRITER: ResourceMiscFlag =
    ResourceMiscFlag(0x200000);

/// The header's `D3D11_RLDO_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_rldo_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RldoFlags(pub i32);

/// The header's `D3D11_RLDO_SUMMARY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_rldo_flags).
pub const D3D11_RLDO_SUMMARY: RldoFlags = RldoFlags(1);
/// The header's `D3D11_RLDO_DETAIL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_rldo_flags).
pub const D3D11_RLDO_DETAIL: RldoFlags = RldoFlags(2);
/// The header's `D3D11_RLDO_IGNORE_INTERNAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_rldo_flags).
pub const D3D11_RLDO_IGNORE_INTERNAL: RldoFlags = RldoFlags(4);

/// The header's `D3D11_RTV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RtvDimension(pub i32);

/// The header's `D3D11_RTV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_UNKNOWN: RtvDimension = RtvDimension(0);
/// The header's `D3D11_RTV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_BUFFER: RtvDimension = RtvDimension(1);
/// The header's `D3D11_RTV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_TEXTURE1D: RtvDimension = RtvDimension(2);
/// The header's `D3D11_RTV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_TEXTURE1DARRAY: RtvDimension = RtvDimension(3);
/// The header's `D3D11_RTV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_TEXTURE2D: RtvDimension = RtvDimension(4);
/// The header's `D3D11_RTV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_TEXTURE2DARRAY: RtvDimension = RtvDimension(5);
/// The header's `D3D11_RTV_DIMENSION_TEXTURE2DMS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_TEXTURE2DMS: RtvDimension = RtvDimension(6);
/// The header's `D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY: RtvDimension = RtvDimension(7);
/// The header's `D3D11_RTV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_rtv_dimension).
pub const D3D11_RTV_DIMENSION_TEXTURE3D: RtvDimension = RtvDimension(8);

/// The header's `D3D11_SHADER_CACHE_SUPPORT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shader_cache_support_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderCacheSupportFlags(pub i32);

/// The header's `D3D11_SHADER_CACHE_SUPPORT_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shader_cache_support_flags).
pub const D3D11_SHADER_CACHE_SUPPORT_NONE: ShaderCacheSupportFlags = ShaderCacheSupportFlags(0);
/// The header's `D3D11_SHADER_CACHE_SUPPORT_AUTOMATIC_INPROC_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shader_cache_support_flags).
pub const D3D11_SHADER_CACHE_SUPPORT_AUTOMATIC_INPROC_CACHE: ShaderCacheSupportFlags =
    ShaderCacheSupportFlags(1);
/// The header's `D3D11_SHADER_CACHE_SUPPORT_AUTOMATIC_DISK_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shader_cache_support_flags).
pub const D3D11_SHADER_CACHE_SUPPORT_AUTOMATIC_DISK_CACHE: ShaderCacheSupportFlags =
    ShaderCacheSupportFlags(2);

/// The header's `D3D11_SHADER_MIN_PRECISION_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shader_min_precision_support).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderMinPrecisionSupport(pub i32);

/// The header's `D3D11_SHADER_MIN_PRECISION_10_BIT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shader_min_precision_support).
pub const D3D11_SHADER_MIN_PRECISION_10_BIT: ShaderMinPrecisionSupport =
    ShaderMinPrecisionSupport(1);
/// The header's `D3D11_SHADER_MIN_PRECISION_16_BIT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shader_min_precision_support).
pub const D3D11_SHADER_MIN_PRECISION_16_BIT: ShaderMinPrecisionSupport =
    ShaderMinPrecisionSupport(2);

/// The header's `D3D11_SHADER_TRACKING_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderTrackingOptions(pub i32);

/// The header's `D3D11_SHADER_TRACKING_OPTION_IGNORE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_IGNORE: ShaderTrackingOptions = ShaderTrackingOptions(0);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_UNINITIALIZED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_UNINITIALIZED: ShaderTrackingOptions =
    ShaderTrackingOptions(1);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_RAW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_RAW: ShaderTrackingOptions = ShaderTrackingOptions(2);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_WAR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_WAR: ShaderTrackingOptions = ShaderTrackingOptions(4);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_WAW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_WAW: ShaderTrackingOptions = ShaderTrackingOptions(8);
/// The header's `D3D11_SHADER_TRACKING_OPTION_ALLOW_SAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_ALLOW_SAME: ShaderTrackingOptions =
    ShaderTrackingOptions(16);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_ATOMIC_CONSISTENCY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_ATOMIC_CONSISTENCY: ShaderTrackingOptions =
    ShaderTrackingOptions(32);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_RAW_ACROSS_THREADGROUPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_RAW_ACROSS_THREADGROUPS: ShaderTrackingOptions =
    ShaderTrackingOptions(64);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_WAR_ACROSS_THREADGROUPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_WAR_ACROSS_THREADGROUPS: ShaderTrackingOptions =
    ShaderTrackingOptions(128);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_WAW_ACROSS_THREADGROUPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_WAW_ACROSS_THREADGROUPS: ShaderTrackingOptions =
    ShaderTrackingOptions(0x100);
/// The header's `D3D11_SHADER_TRACKING_OPTION_TRACK_ATOMIC_CONSISTENCY_ACROSS_THREADGROUPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_TRACK_ATOMIC_CONSISTENCY_ACROSS_THREADGROUPS:
    ShaderTrackingOptions = ShaderTrackingOptions(0x200);
/// The header's `D3D11_SHADER_TRACKING_OPTION_UAV_SPECIFIC_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_UAV_SPECIFIC_FLAGS: ShaderTrackingOptions =
    ShaderTrackingOptions(0x3c0);
/// The header's `D3D11_SHADER_TRACKING_OPTION_ALL_HAZARDS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_ALL_HAZARDS: ShaderTrackingOptions =
    ShaderTrackingOptions(0x3ee);
/// The header's `D3D11_SHADER_TRACKING_OPTION_ALL_HAZARDS_ALLOWING_SAME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_ALL_HAZARDS_ALLOWING_SAME: ShaderTrackingOptions =
    ShaderTrackingOptions(0x3fe);
/// The header's `D3D11_SHADER_TRACKING_OPTION_ALL_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_options).
pub const D3D11_SHADER_TRACKING_OPTION_ALL_OPTIONS: ShaderTrackingOptions =
    ShaderTrackingOptions(0x3ff);

/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderTrackingResourceType(pub i32);

/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
pub const D3D11_SHADER_TRACKING_RESOURCE_TYPE_NONE: ShaderTrackingResourceType =
    ShaderTrackingResourceType(0);
/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE_UAV_DEVICEMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
pub const D3D11_SHADER_TRACKING_RESOURCE_TYPE_UAV_DEVICEMEMORY: ShaderTrackingResourceType =
    ShaderTrackingResourceType(1);
/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE_NON_UAV_DEVICEMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
pub const D3D11_SHADER_TRACKING_RESOURCE_TYPE_NON_UAV_DEVICEMEMORY: ShaderTrackingResourceType =
    ShaderTrackingResourceType(2);
/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE_ALL_DEVICEMEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
pub const D3D11_SHADER_TRACKING_RESOURCE_TYPE_ALL_DEVICEMEMORY: ShaderTrackingResourceType =
    ShaderTrackingResourceType(3);
/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE_GROUPSHARED_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
pub const D3D11_SHADER_TRACKING_RESOURCE_TYPE_GROUPSHARED_MEMORY: ShaderTrackingResourceType =
    ShaderTrackingResourceType(4);
/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE_ALL_SHARED_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
pub const D3D11_SHADER_TRACKING_RESOURCE_TYPE_ALL_SHARED_MEMORY: ShaderTrackingResourceType =
    ShaderTrackingResourceType(5);
/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE_GROUPSHARED_NON_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
pub const D3D11_SHADER_TRACKING_RESOURCE_TYPE_GROUPSHARED_NON_UAV: ShaderTrackingResourceType =
    ShaderTrackingResourceType(6);
/// The header's `D3D11_SHADER_TRACKING_RESOURCE_TYPE_ALL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ne-d3d11sdklayers-d3d11_shader_tracking_resource_type).
pub const D3D11_SHADER_TRACKING_RESOURCE_TYPE_ALL: ShaderTrackingResourceType =
    ShaderTrackingResourceType(7);

/// The header's `D3D11_SHADER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_shader_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderType(pub i32);

/// The header's `D3D11_VERTEX_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_shader_type).
pub const D3D11_VERTEX_SHADER: ShaderType = ShaderType(1);
/// The header's `D3D11_HULL_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_shader_type).
pub const D3D11_HULL_SHADER: ShaderType = ShaderType(2);
/// The header's `D3D11_DOMAIN_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_shader_type).
pub const D3D11_DOMAIN_SHADER: ShaderType = ShaderType(3);
/// The header's `D3D11_GEOMETRY_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_shader_type).
pub const D3D11_GEOMETRY_SHADER: ShaderType = ShaderType(4);
/// The header's `D3D11_PIXEL_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_shader_type).
pub const D3D11_PIXEL_SHADER: ShaderType = ShaderType(5);
/// The header's `D3D11_COMPUTE_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_shader_type).
pub const D3D11_COMPUTE_SHADER: ShaderType = ShaderType(6);

/// The header's `D3D11_SHADER_VERSION_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ne-d3d11shader-d3d11_shader_version_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShaderVersionType(pub i32);

/// The header's `D3D11_SHVER_PIXEL_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ne-d3d11shader-d3d11_shader_version_type).
pub const D3D11_SHVER_PIXEL_SHADER: ShaderVersionType = ShaderVersionType(0);
/// The header's `D3D11_SHVER_VERTEX_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ne-d3d11shader-d3d11_shader_version_type).
pub const D3D11_SHVER_VERTEX_SHADER: ShaderVersionType = ShaderVersionType(1);
/// The header's `D3D11_SHVER_GEOMETRY_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ne-d3d11shader-d3d11_shader_version_type).
pub const D3D11_SHVER_GEOMETRY_SHADER: ShaderVersionType = ShaderVersionType(2);
/// The header's `D3D11_SHVER_HULL_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ne-d3d11shader-d3d11_shader_version_type).
pub const D3D11_SHVER_HULL_SHADER: ShaderVersionType = ShaderVersionType(3);
/// The header's `D3D11_SHVER_DOMAIN_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ne-d3d11shader-d3d11_shader_version_type).
pub const D3D11_SHVER_DOMAIN_SHADER: ShaderVersionType = ShaderVersionType(4);
/// The header's `D3D11_SHVER_COMPUTE_SHADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ne-d3d11shader-d3d11_shader_version_type).
pub const D3D11_SHVER_COMPUTE_SHADER: ShaderVersionType = ShaderVersionType(5);
/// The header's `D3D11_SHVER_RESERVED0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ne-d3d11shader-d3d11_shader_version_type).
pub const D3D11_SHVER_RESERVED0: ShaderVersionType = ShaderVersionType(0xfff0);

/// The header's `D3D11_SHARED_RESOURCE_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shared_resource_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SharedResourceTier(pub i32);

/// The header's `D3D11_SHARED_RESOURCE_TIER_0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shared_resource_tier).
pub const D3D11_SHARED_RESOURCE_TIER_0: SharedResourceTier = SharedResourceTier(0);
/// The header's `D3D11_SHARED_RESOURCE_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shared_resource_tier).
pub const D3D11_SHARED_RESOURCE_TIER_1: SharedResourceTier = SharedResourceTier(1);
/// The header's `D3D11_SHARED_RESOURCE_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shared_resource_tier).
pub const D3D11_SHARED_RESOURCE_TIER_2: SharedResourceTier = SharedResourceTier(2);
/// The header's `D3D11_SHARED_RESOURCE_TIER_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_shared_resource_tier).
pub const D3D11_SHARED_RESOURCE_TIER_3: SharedResourceTier = SharedResourceTier(3);

/// The header's `D3D11_STANDARD_MULTISAMPLE_QUALITY_LEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_standard_multisample_quality_levels).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StandardMultisampleQualityLevels(pub i32);

/// The header's `D3D11_STANDARD_MULTISAMPLE_PATTERN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_standard_multisample_quality_levels).
pub const D3D11_STANDARD_MULTISAMPLE_PATTERN: StandardMultisampleQualityLevels =
    StandardMultisampleQualityLevels(-1);
/// The header's `D3D11_CENTER_MULTISAMPLE_PATTERN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_standard_multisample_quality_levels).
pub const D3D11_CENTER_MULTISAMPLE_PATTERN: StandardMultisampleQualityLevels =
    StandardMultisampleQualityLevels(-2);

/// The header's `D3D11_STENCIL_OP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StencilOp(pub i32);

/// The header's `D3D11_STENCIL_OP_KEEP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
pub const D3D11_STENCIL_OP_KEEP: StencilOp = StencilOp(1);
/// The header's `D3D11_STENCIL_OP_ZERO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
pub const D3D11_STENCIL_OP_ZERO: StencilOp = StencilOp(2);
/// The header's `D3D11_STENCIL_OP_REPLACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
pub const D3D11_STENCIL_OP_REPLACE: StencilOp = StencilOp(3);
/// The header's `D3D11_STENCIL_OP_INCR_SAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
pub const D3D11_STENCIL_OP_INCR_SAT: StencilOp = StencilOp(4);
/// The header's `D3D11_STENCIL_OP_DECR_SAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
pub const D3D11_STENCIL_OP_DECR_SAT: StencilOp = StencilOp(5);
/// The header's `D3D11_STENCIL_OP_INVERT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
pub const D3D11_STENCIL_OP_INVERT: StencilOp = StencilOp(6);
/// The header's `D3D11_STENCIL_OP_INCR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
pub const D3D11_STENCIL_OP_INCR: StencilOp = StencilOp(7);
/// The header's `D3D11_STENCIL_OP_DECR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_stencil_op).
pub const D3D11_STENCIL_OP_DECR: StencilOp = StencilOp(8);

/// The header's `D3D11_TEXTURECUBE_FACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texturecube_face).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TexturecubeFace(pub i32);

/// The header's `D3D11_TEXTURECUBE_FACE_POSITIVE_X`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texturecube_face).
pub const D3D11_TEXTURECUBE_FACE_POSITIVE_X: TexturecubeFace = TexturecubeFace(0);
/// The header's `D3D11_TEXTURECUBE_FACE_NEGATIVE_X`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texturecube_face).
pub const D3D11_TEXTURECUBE_FACE_NEGATIVE_X: TexturecubeFace = TexturecubeFace(1);
/// The header's `D3D11_TEXTURECUBE_FACE_POSITIVE_Y`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texturecube_face).
pub const D3D11_TEXTURECUBE_FACE_POSITIVE_Y: TexturecubeFace = TexturecubeFace(2);
/// The header's `D3D11_TEXTURECUBE_FACE_NEGATIVE_Y`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texturecube_face).
pub const D3D11_TEXTURECUBE_FACE_NEGATIVE_Y: TexturecubeFace = TexturecubeFace(3);
/// The header's `D3D11_TEXTURECUBE_FACE_POSITIVE_Z`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texturecube_face).
pub const D3D11_TEXTURECUBE_FACE_POSITIVE_Z: TexturecubeFace = TexturecubeFace(4);
/// The header's `D3D11_TEXTURECUBE_FACE_NEGATIVE_Z`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texturecube_face).
pub const D3D11_TEXTURECUBE_FACE_NEGATIVE_Z: TexturecubeFace = TexturecubeFace(5);

/// The header's `D3D11_TEXTURE_ADDRESS_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texture_address_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureAddressMode(pub i32);

/// The header's `D3D11_TEXTURE_ADDRESS_WRAP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texture_address_mode).
pub const D3D11_TEXTURE_ADDRESS_WRAP: TextureAddressMode = TextureAddressMode(1);
/// The header's `D3D11_TEXTURE_ADDRESS_MIRROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texture_address_mode).
pub const D3D11_TEXTURE_ADDRESS_MIRROR: TextureAddressMode = TextureAddressMode(2);
/// The header's `D3D11_TEXTURE_ADDRESS_CLAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texture_address_mode).
pub const D3D11_TEXTURE_ADDRESS_CLAMP: TextureAddressMode = TextureAddressMode(3);
/// The header's `D3D11_TEXTURE_ADDRESS_BORDER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texture_address_mode).
pub const D3D11_TEXTURE_ADDRESS_BORDER: TextureAddressMode = TextureAddressMode(4);
/// The header's `D3D11_TEXTURE_ADDRESS_MIRROR_ONCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_texture_address_mode).
pub const D3D11_TEXTURE_ADDRESS_MIRROR_ONCE: TextureAddressMode = TextureAddressMode(5);

/// The header's `D3D11_TEXTURE_LAYOUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_texture_layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureLayout(pub i32);

/// The header's `D3D11_TEXTURE_LAYOUT_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_texture_layout).
pub const D3D11_TEXTURE_LAYOUT_UNDEFINED: TextureLayout = TextureLayout(0);
/// The header's `D3D11_TEXTURE_LAYOUT_ROW_MAJOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_texture_layout).
pub const D3D11_TEXTURE_LAYOUT_ROW_MAJOR: TextureLayout = TextureLayout(1);
/// The header's `D3D11_TEXTURE_LAYOUT_64K_STANDARD_SWIZZLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ne-d3d11_3-d3d11_texture_layout).
pub const D3D11_TEXTURE_LAYOUT_64K_STANDARD_SWIZZLE: TextureLayout = TextureLayout(2);

/// The header's `D3D11_TILED_RESOURCES_TIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_tiled_resources_tier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TiledResourcesTier(pub i32);

/// The header's `D3D11_TILED_RESOURCES_NOT_SUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_tiled_resources_tier).
pub const D3D11_TILED_RESOURCES_NOT_SUPPORTED: TiledResourcesTier = TiledResourcesTier(0);
/// The header's `D3D11_TILED_RESOURCES_TIER_1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_tiled_resources_tier).
pub const D3D11_TILED_RESOURCES_TIER_1: TiledResourcesTier = TiledResourcesTier(1);
/// The header's `D3D11_TILED_RESOURCES_TIER_2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_tiled_resources_tier).
pub const D3D11_TILED_RESOURCES_TIER_2: TiledResourcesTier = TiledResourcesTier(2);
/// The header's `D3D11_TILED_RESOURCES_TIER_3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_tiled_resources_tier).
pub const D3D11_TILED_RESOURCES_TIER_3: TiledResourcesTier = TiledResourcesTier(3);

/// The header's `D3D11_TILE_COPY_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_copy_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TileCopyFlag(pub i32);

/// The header's `D3D11_TILE_COPY_NO_OVERWRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_copy_flag).
pub const D3D11_TILE_COPY_NO_OVERWRITE: TileCopyFlag = TileCopyFlag(1);
/// The header's `D3D11_TILE_COPY_LINEAR_BUFFER_TO_SWIZZLED_TILED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_copy_flag).
pub const D3D11_TILE_COPY_LINEAR_BUFFER_TO_SWIZZLED_TILED_RESOURCE: TileCopyFlag = TileCopyFlag(2);
/// The header's `D3D11_TILE_COPY_SWIZZLED_TILED_RESOURCE_TO_LINEAR_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_copy_flag).
pub const D3D11_TILE_COPY_SWIZZLED_TILED_RESOURCE_TO_LINEAR_BUFFER: TileCopyFlag = TileCopyFlag(4);

/// The header's `D3D11_TILE_MAPPING_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_mapping_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TileMappingFlag(pub i32);

/// The header's `D3D11_TILE_MAPPING_NO_OVERWRITE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_mapping_flag).
pub const D3D11_TILE_MAPPING_NO_OVERWRITE: TileMappingFlag = TileMappingFlag(1);

/// The header's `D3D11_TILE_RANGE_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_range_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TileRangeFlag(pub i32);

/// The header's `D3D11_TILE_RANGE_NULL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_range_flag).
pub const D3D11_TILE_RANGE_NULL: TileRangeFlag = TileRangeFlag(1);
/// The header's `D3D11_TILE_RANGE_SKIP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_range_flag).
pub const D3D11_TILE_RANGE_SKIP: TileRangeFlag = TileRangeFlag(2);
/// The header's `D3D11_TILE_RANGE_REUSE_SINGLE_TILE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ne-d3d11_2-d3d11_tile_range_flag).
pub const D3D11_TILE_RANGE_REUSE_SINGLE_TILE: TileRangeFlag = TileRangeFlag(4);

/// The header's `D3D11_TRACE_GS_INPUT_PRIMITIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_gs_input_primitive).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TraceGsInputPrimitive(pub i32);

/// The header's `D3D11_TRACE_GS_INPUT_PRIMITIVE_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_gs_input_primitive).
pub const D3D11_TRACE_GS_INPUT_PRIMITIVE_UNDEFINED: TraceGsInputPrimitive =
    TraceGsInputPrimitive(0);
/// The header's `D3D11_TRACE_GS_INPUT_PRIMITIVE_POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_gs_input_primitive).
pub const D3D11_TRACE_GS_INPUT_PRIMITIVE_POINT: TraceGsInputPrimitive = TraceGsInputPrimitive(1);
/// The header's `D3D11_TRACE_GS_INPUT_PRIMITIVE_LINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_gs_input_primitive).
pub const D3D11_TRACE_GS_INPUT_PRIMITIVE_LINE: TraceGsInputPrimitive = TraceGsInputPrimitive(2);
/// The header's `D3D11_TRACE_GS_INPUT_PRIMITIVE_TRIANGLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_gs_input_primitive).
pub const D3D11_TRACE_GS_INPUT_PRIMITIVE_TRIANGLE: TraceGsInputPrimitive = TraceGsInputPrimitive(3);
/// The header's `D3D11_TRACE_GS_INPUT_PRIMITIVE_LINE_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_gs_input_primitive).
pub const D3D11_TRACE_GS_INPUT_PRIMITIVE_LINE_ADJ: TraceGsInputPrimitive = TraceGsInputPrimitive(6);
/// The header's `D3D11_TRACE_GS_INPUT_PRIMITIVE_TRIANGLE_ADJ`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_gs_input_primitive).
pub const D3D11_TRACE_GS_INPUT_PRIMITIVE_TRIANGLE_ADJ: TraceGsInputPrimitive =
    TraceGsInputPrimitive(7);

/// The header's `D3D11_TRACE_REGISTER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TraceRegisterType(pub i32);

/// The header's `D3D11_TRACE_OUTPUT_NULL_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_OUTPUT_NULL_REGISTER: TraceRegisterType = TraceRegisterType(0);
/// The header's `D3D11_TRACE_INPUT_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_REGISTER: TraceRegisterType = TraceRegisterType(1);
/// The header's `D3D11_TRACE_INPUT_PRIMITIVE_ID_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_PRIMITIVE_ID_REGISTER: TraceRegisterType = TraceRegisterType(2);
/// The header's `D3D11_TRACE_IMMEDIATE_CONSTANT_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_IMMEDIATE_CONSTANT_BUFFER: TraceRegisterType = TraceRegisterType(3);
/// The header's `D3D11_TRACE_TEMP_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_TEMP_REGISTER: TraceRegisterType = TraceRegisterType(4);
/// The header's `D3D11_TRACE_INDEXABLE_TEMP_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INDEXABLE_TEMP_REGISTER: TraceRegisterType = TraceRegisterType(5);
/// The header's `D3D11_TRACE_OUTPUT_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_OUTPUT_REGISTER: TraceRegisterType = TraceRegisterType(6);
/// The header's `D3D11_TRACE_OUTPUT_DEPTH_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_OUTPUT_DEPTH_REGISTER: TraceRegisterType = TraceRegisterType(7);
/// The header's `D3D11_TRACE_CONSTANT_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_CONSTANT_BUFFER: TraceRegisterType = TraceRegisterType(8);
/// The header's `D3D11_TRACE_IMMEDIATE32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_IMMEDIATE32: TraceRegisterType = TraceRegisterType(9);
/// The header's `D3D11_TRACE_SAMPLER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_SAMPLER: TraceRegisterType = TraceRegisterType(10);
/// The header's `D3D11_TRACE_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_RESOURCE: TraceRegisterType = TraceRegisterType(11);
/// The header's `D3D11_TRACE_RASTERIZER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_RASTERIZER: TraceRegisterType = TraceRegisterType(12);
/// The header's `D3D11_TRACE_OUTPUT_COVERAGE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_OUTPUT_COVERAGE_MASK: TraceRegisterType = TraceRegisterType(13);
/// The header's `D3D11_TRACE_STREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_STREAM: TraceRegisterType = TraceRegisterType(14);
/// The header's `D3D11_TRACE_THIS_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_THIS_POINTER: TraceRegisterType = TraceRegisterType(15);
/// The header's `D3D11_TRACE_OUTPUT_CONTROL_POINT_ID_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_OUTPUT_CONTROL_POINT_ID_REGISTER: TraceRegisterType = TraceRegisterType(16);
/// The header's `D3D11_TRACE_INPUT_FORK_INSTANCE_ID_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_FORK_INSTANCE_ID_REGISTER: TraceRegisterType = TraceRegisterType(17);
/// The header's `D3D11_TRACE_INPUT_JOIN_INSTANCE_ID_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_JOIN_INSTANCE_ID_REGISTER: TraceRegisterType = TraceRegisterType(18);
/// The header's `D3D11_TRACE_INPUT_CONTROL_POINT_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_CONTROL_POINT_REGISTER: TraceRegisterType = TraceRegisterType(19);
/// The header's `D3D11_TRACE_OUTPUT_CONTROL_POINT_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_OUTPUT_CONTROL_POINT_REGISTER: TraceRegisterType = TraceRegisterType(20);
/// The header's `D3D11_TRACE_INPUT_PATCH_CONSTANT_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_PATCH_CONSTANT_REGISTER: TraceRegisterType = TraceRegisterType(21);
/// The header's `D3D11_TRACE_INPUT_DOMAIN_POINT_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_DOMAIN_POINT_REGISTER: TraceRegisterType = TraceRegisterType(22);
/// The header's `D3D11_TRACE_UNORDERED_ACCESS_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_UNORDERED_ACCESS_VIEW: TraceRegisterType = TraceRegisterType(23);
/// The header's `D3D11_TRACE_THREAD_GROUP_SHARED_MEMORY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_THREAD_GROUP_SHARED_MEMORY: TraceRegisterType = TraceRegisterType(24);
/// The header's `D3D11_TRACE_INPUT_THREAD_ID_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_THREAD_ID_REGISTER: TraceRegisterType = TraceRegisterType(25);
/// The header's `D3D11_TRACE_INPUT_THREAD_GROUP_ID_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_THREAD_GROUP_ID_REGISTER: TraceRegisterType = TraceRegisterType(26);
/// The header's `D3D11_TRACE_INPUT_THREAD_ID_IN_GROUP_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_THREAD_ID_IN_GROUP_REGISTER: TraceRegisterType = TraceRegisterType(27);
/// The header's `D3D11_TRACE_INPUT_COVERAGE_MASK_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_COVERAGE_MASK_REGISTER: TraceRegisterType = TraceRegisterType(28);
/// The header's `D3D11_TRACE_INPUT_THREAD_ID_IN_GROUP_FLATTENED_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_THREAD_ID_IN_GROUP_FLATTENED_REGISTER: TraceRegisterType =
    TraceRegisterType(29);
/// The header's `D3D11_TRACE_INPUT_GS_INSTANCE_ID_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_GS_INSTANCE_ID_REGISTER: TraceRegisterType = TraceRegisterType(30);
/// The header's `D3D11_TRACE_OUTPUT_DEPTH_GREATER_EQUAL_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_OUTPUT_DEPTH_GREATER_EQUAL_REGISTER: TraceRegisterType =
    TraceRegisterType(31);
/// The header's `D3D11_TRACE_OUTPUT_DEPTH_LESS_EQUAL_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_OUTPUT_DEPTH_LESS_EQUAL_REGISTER: TraceRegisterType = TraceRegisterType(32);
/// The header's `D3D11_TRACE_IMMEDIATE64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_IMMEDIATE64: TraceRegisterType = TraceRegisterType(33);
/// The header's `D3D11_TRACE_INPUT_CYCLE_COUNTER_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INPUT_CYCLE_COUNTER_REGISTER: TraceRegisterType = TraceRegisterType(34);
/// The header's `D3D11_TRACE_INTERFACE_POINTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ne-d3d11shadertracing-d3d11_trace_register_type).
pub const D3D11_TRACE_INTERFACE_POINTER: TraceRegisterType = TraceRegisterType(35);

/// The header's `D3D11_UAV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_uav_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UavDimension(pub i32);

/// The header's `D3D11_UAV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_uav_dimension).
pub const D3D11_UAV_DIMENSION_UNKNOWN: UavDimension = UavDimension(0);
/// The header's `D3D11_UAV_DIMENSION_BUFFER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_uav_dimension).
pub const D3D11_UAV_DIMENSION_BUFFER: UavDimension = UavDimension(1);
/// The header's `D3D11_UAV_DIMENSION_TEXTURE1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_uav_dimension).
pub const D3D11_UAV_DIMENSION_TEXTURE1D: UavDimension = UavDimension(2);
/// The header's `D3D11_UAV_DIMENSION_TEXTURE1DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_uav_dimension).
pub const D3D11_UAV_DIMENSION_TEXTURE1DARRAY: UavDimension = UavDimension(3);
/// The header's `D3D11_UAV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_uav_dimension).
pub const D3D11_UAV_DIMENSION_TEXTURE2D: UavDimension = UavDimension(4);
/// The header's `D3D11_UAV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_uav_dimension).
pub const D3D11_UAV_DIMENSION_TEXTURE2DARRAY: UavDimension = UavDimension(5);
/// The header's `D3D11_UAV_DIMENSION_TEXTURE3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_uav_dimension).
pub const D3D11_UAV_DIMENSION_TEXTURE3D: UavDimension = UavDimension(8);

/// The header's `D3D11_USAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_usage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Usage(pub i32);

/// The header's `D3D11_USAGE_DEFAULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_usage).
pub const D3D11_USAGE_DEFAULT: Usage = Usage(0);
/// The header's `D3D11_USAGE_IMMUTABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_usage).
pub const D3D11_USAGE_IMMUTABLE: Usage = Usage(1);
/// The header's `D3D11_USAGE_DYNAMIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_usage).
pub const D3D11_USAGE_DYNAMIC: Usage = Usage(2);
/// The header's `D3D11_USAGE_STAGING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_usage).
pub const D3D11_USAGE_STAGING: Usage = Usage(3);

/// The header's `D3D11_VDOV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vdov_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VdovDimension(pub i32);

/// The header's `D3D11_VDOV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vdov_dimension).
pub const D3D11_VDOV_DIMENSION_UNKNOWN: VdovDimension = VdovDimension(0);
/// The header's `D3D11_VDOV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vdov_dimension).
pub const D3D11_VDOV_DIMENSION_TEXTURE2D: VdovDimension = VdovDimension(1);

/// The header's `D3D11_VIDEO_DECODER_BUFFER_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoDecoderBufferType(pub i32);

/// The header's `D3D11_VIDEO_DECODER_BUFFER_PICTURE_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_PICTURE_PARAMETERS: VideoDecoderBufferType =
    VideoDecoderBufferType(0);
/// The header's `D3D11_VIDEO_DECODER_BUFFER_MACROBLOCK_CONTROL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_MACROBLOCK_CONTROL: VideoDecoderBufferType =
    VideoDecoderBufferType(1);
/// The header's `D3D11_VIDEO_DECODER_BUFFER_RESIDUAL_DIFFERENCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_RESIDUAL_DIFFERENCE: VideoDecoderBufferType =
    VideoDecoderBufferType(2);
/// The header's `D3D11_VIDEO_DECODER_BUFFER_DEBLOCKING_CONTROL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_DEBLOCKING_CONTROL: VideoDecoderBufferType =
    VideoDecoderBufferType(3);
/// The header's `D3D11_VIDEO_DECODER_BUFFER_INVERSE_QUANTIZATION_MATRIX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_INVERSE_QUANTIZATION_MATRIX: VideoDecoderBufferType =
    VideoDecoderBufferType(4);
/// The header's `D3D11_VIDEO_DECODER_BUFFER_SLICE_CONTROL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_SLICE_CONTROL: VideoDecoderBufferType =
    VideoDecoderBufferType(5);
/// The header's `D3D11_VIDEO_DECODER_BUFFER_BITSTREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_BITSTREAM: VideoDecoderBufferType = VideoDecoderBufferType(6);
/// The header's `D3D11_VIDEO_DECODER_BUFFER_MOTION_VECTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_MOTION_VECTOR: VideoDecoderBufferType =
    VideoDecoderBufferType(7);
/// The header's `D3D11_VIDEO_DECODER_BUFFER_FILM_GRAIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_decoder_buffer_type).
pub const D3D11_VIDEO_DECODER_BUFFER_FILM_GRAIN: VideoDecoderBufferType = VideoDecoderBufferType(8);

/// The header's `D3D11_VIDEO_DECODER_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_decoder_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoDecoderCaps(pub i32);

/// The header's `D3D11_VIDEO_DECODER_CAPS_DOWNSAMPLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_decoder_caps).
pub const D3D11_VIDEO_DECODER_CAPS_DOWNSAMPLE: VideoDecoderCaps = VideoDecoderCaps(1);
/// The header's `D3D11_VIDEO_DECODER_CAPS_NON_REAL_TIME`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_decoder_caps).
pub const D3D11_VIDEO_DECODER_CAPS_NON_REAL_TIME: VideoDecoderCaps = VideoDecoderCaps(2);
/// The header's `D3D11_VIDEO_DECODER_CAPS_DOWNSAMPLE_DYNAMIC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_decoder_caps).
pub const D3D11_VIDEO_DECODER_CAPS_DOWNSAMPLE_DYNAMIC: VideoDecoderCaps = VideoDecoderCaps(4);
/// The header's `D3D11_VIDEO_DECODER_CAPS_DOWNSAMPLE_REQUIRED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_decoder_caps).
pub const D3D11_VIDEO_DECODER_CAPS_DOWNSAMPLE_REQUIRED: VideoDecoderCaps = VideoDecoderCaps(8);
/// The header's `D3D11_VIDEO_DECODER_CAPS_UNSUPPORTED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_decoder_caps).
pub const D3D11_VIDEO_DECODER_CAPS_UNSUPPORTED: VideoDecoderCaps = VideoDecoderCaps(16);

/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoDecoderHistogramComponent(pub i32);

/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_Y`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_Y: VideoDecoderHistogramComponent =
    VideoDecoderHistogramComponent(0);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_U`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_U: VideoDecoderHistogramComponent =
    VideoDecoderHistogramComponent(1);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_V`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_V: VideoDecoderHistogramComponent =
    VideoDecoderHistogramComponent(2);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_R`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_R: VideoDecoderHistogramComponent =
    VideoDecoderHistogramComponent(0);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_G`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_G: VideoDecoderHistogramComponent =
    VideoDecoderHistogramComponent(1);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_B`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_B: VideoDecoderHistogramComponent =
    VideoDecoderHistogramComponent(2);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_A`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_A: VideoDecoderHistogramComponent =
    VideoDecoderHistogramComponent(3);

/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoDecoderHistogramComponentFlags(pub i32);

/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_NONE: VideoDecoderHistogramComponentFlags =
    VideoDecoderHistogramComponentFlags(0);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_Y`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_Y: VideoDecoderHistogramComponentFlags =
    VideoDecoderHistogramComponentFlags(1);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_U`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_U: VideoDecoderHistogramComponentFlags =
    VideoDecoderHistogramComponentFlags(2);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_V`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_V: VideoDecoderHistogramComponentFlags =
    VideoDecoderHistogramComponentFlags(4);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_R`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_R: VideoDecoderHistogramComponentFlags =
    VideoDecoderHistogramComponentFlags(1);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_G`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_G: VideoDecoderHistogramComponentFlags =
    VideoDecoderHistogramComponentFlags(2);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_B`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_B: VideoDecoderHistogramComponentFlags =
    VideoDecoderHistogramComponentFlags(4);
/// The header's `D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_A`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ne-d3d11_4-d3d11_video_decoder_histogram_component_flags).
pub const D3D11_VIDEO_DECODER_HISTOGRAM_COMPONENT_FLAG_A: VideoDecoderHistogramComponentFlags =
    VideoDecoderHistogramComponentFlags(8);

/// The header's `D3D11_VIDEO_FRAME_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_frame_format).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoFrameFormat(pub i32);

/// The header's `D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_frame_format).
pub const D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE: VideoFrameFormat = VideoFrameFormat(0);
/// The header's `D3D11_VIDEO_FRAME_FORMAT_INTERLACED_TOP_FIELD_FIRST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_frame_format).
pub const D3D11_VIDEO_FRAME_FORMAT_INTERLACED_TOP_FIELD_FIRST: VideoFrameFormat =
    VideoFrameFormat(1);
/// The header's `D3D11_VIDEO_FRAME_FORMAT_INTERLACED_BOTTOM_FIELD_FIRST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_frame_format).
pub const D3D11_VIDEO_FRAME_FORMAT_INTERLACED_BOTTOM_FIELD_FIRST: VideoFrameFormat =
    VideoFrameFormat(2);

/// The header's `D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_alpha_fill_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorAlphaFillMode(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_OPAQUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_alpha_fill_mode).
pub const D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_OPAQUE: VideoProcessorAlphaFillMode =
    VideoProcessorAlphaFillMode(0);
/// The header's `D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_BACKGROUND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_alpha_fill_mode).
pub const D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_BACKGROUND: VideoProcessorAlphaFillMode =
    VideoProcessorAlphaFillMode(1);
/// The header's `D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_DESTINATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_alpha_fill_mode).
pub const D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_DESTINATION: VideoProcessorAlphaFillMode =
    VideoProcessorAlphaFillMode(2);
/// The header's `D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_SOURCE_STREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_alpha_fill_mode).
pub const D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_SOURCE_STREAM: VideoProcessorAlphaFillMode =
    VideoProcessorAlphaFillMode(3);

/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorAutoStreamCaps(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_DENOISE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
pub const D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_DENOISE: VideoProcessorAutoStreamCaps =
    VideoProcessorAutoStreamCaps(1);
/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_DERINGING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
pub const D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_DERINGING: VideoProcessorAutoStreamCaps =
    VideoProcessorAutoStreamCaps(2);
/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_EDGE_ENHANCEMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
pub const D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_EDGE_ENHANCEMENT: VideoProcessorAutoStreamCaps =
    VideoProcessorAutoStreamCaps(4);
/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_COLOR_CORRECTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
pub const D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_COLOR_CORRECTION: VideoProcessorAutoStreamCaps =
    VideoProcessorAutoStreamCaps(8);
/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_FLESH_TONE_MAPPING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
pub const D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_FLESH_TONE_MAPPING: VideoProcessorAutoStreamCaps =
    VideoProcessorAutoStreamCaps(16);
/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_IMAGE_STABILIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
pub const D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_IMAGE_STABILIZATION: VideoProcessorAutoStreamCaps =
    VideoProcessorAutoStreamCaps(32);
/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_SUPER_RESOLUTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
pub const D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_SUPER_RESOLUTION: VideoProcessorAutoStreamCaps =
    VideoProcessorAutoStreamCaps(64);
/// The header's `D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_ANAMORPHIC_SCALING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_auto_stream_caps).
pub const D3D11_VIDEO_PROCESSOR_AUTO_STREAM_CAPS_ANAMORPHIC_SCALING: VideoProcessorAutoStreamCaps =
    VideoProcessorAutoStreamCaps(128);

/// The header's `D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_processor_behavior_hints).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorBehaviorHints(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINT_MULTIPLANE_OVERLAY_ROTATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_processor_behavior_hints).
pub const D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINT_MULTIPLANE_OVERLAY_ROTATION:
    VideoProcessorBehaviorHints = VideoProcessorBehaviorHints(1);
/// The header's `D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINT_MULTIPLANE_OVERLAY_RESIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_processor_behavior_hints).
pub const D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINT_MULTIPLANE_OVERLAY_RESIZE:
    VideoProcessorBehaviorHints = VideoProcessorBehaviorHints(2);
/// The header's `D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINT_MULTIPLANE_OVERLAY_COLOR_SPACE_CONVERSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_processor_behavior_hints).
pub const D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINT_MULTIPLANE_OVERLAY_COLOR_SPACE_CONVERSION:
    VideoProcessorBehaviorHints = VideoProcessorBehaviorHints(4);
/// The header's `D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINT_TRIPLE_BUFFER_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_video_processor_behavior_hints).
pub const D3D11_VIDEO_PROCESSOR_BEHAVIOR_HINT_TRIPLE_BUFFER_OUTPUT: VideoProcessorBehaviorHints =
    VideoProcessorBehaviorHints(8);

/// The header's `D3D11_VIDEO_PROCESSOR_DEVICE_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_device_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorDeviceCaps(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_LINEAR_SPACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_device_caps).
pub const D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_LINEAR_SPACE: VideoProcessorDeviceCaps =
    VideoProcessorDeviceCaps(1);
/// The header's `D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_xvYCC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_device_caps).
pub const D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_XVYCC: VideoProcessorDeviceCaps =
    VideoProcessorDeviceCaps(2);
/// The header's `D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_RGB_RANGE_CONVERSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_device_caps).
pub const D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_RGB_RANGE_CONVERSION: VideoProcessorDeviceCaps =
    VideoProcessorDeviceCaps(4);
/// The header's `D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_YCbCr_MATRIX_CONVERSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_device_caps).
pub const D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_YCBCR_MATRIX_CONVERSION: VideoProcessorDeviceCaps =
    VideoProcessorDeviceCaps(8);
/// The header's `D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_NOMINAL_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_device_caps).
pub const D3D11_VIDEO_PROCESSOR_DEVICE_CAPS_NOMINAL_RANGE: VideoProcessorDeviceCaps =
    VideoProcessorDeviceCaps(16);

/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorFeatureCaps(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ALPHA_FILL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ALPHA_FILL: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(1);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_CONSTRICTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_CONSTRICTION: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(2);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_LUMA_KEY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_LUMA_KEY: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(4);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ALPHA_PALETTE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ALPHA_PALETTE: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(8);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_LEGACY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_LEGACY: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(16);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_STEREO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_STEREO: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(32);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ROTATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ROTATION: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(64);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ALPHA_STREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_ALPHA_STREAM: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(128);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_PIXEL_ASPECT_RATIO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_PIXEL_ASPECT_RATIO: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(0x100);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_MIRROR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_MIRROR: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(0x200);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_SHADER_USAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_SHADER_USAGE: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(0x400);
/// The header's `D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_METADATA_HDR10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_feature_caps).
pub const D3D11_VIDEO_PROCESSOR_FEATURE_CAPS_METADATA_HDR10: VideoProcessorFeatureCaps =
    VideoProcessorFeatureCaps(0x800);

/// The header's `D3D11_VIDEO_PROCESSOR_FILTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorFilter(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_BRIGHTNESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
pub const D3D11_VIDEO_PROCESSOR_FILTER_BRIGHTNESS: VideoProcessorFilter = VideoProcessorFilter(0);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CONTRAST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CONTRAST: VideoProcessorFilter = VideoProcessorFilter(1);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_HUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
pub const D3D11_VIDEO_PROCESSOR_FILTER_HUE: VideoProcessorFilter = VideoProcessorFilter(2);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_SATURATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
pub const D3D11_VIDEO_PROCESSOR_FILTER_SATURATION: VideoProcessorFilter = VideoProcessorFilter(3);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_NOISE_REDUCTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
pub const D3D11_VIDEO_PROCESSOR_FILTER_NOISE_REDUCTION: VideoProcessorFilter =
    VideoProcessorFilter(4);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_EDGE_ENHANCEMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
pub const D3D11_VIDEO_PROCESSOR_FILTER_EDGE_ENHANCEMENT: VideoProcessorFilter =
    VideoProcessorFilter(5);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_ANAMORPHIC_SCALING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
pub const D3D11_VIDEO_PROCESSOR_FILTER_ANAMORPHIC_SCALING: VideoProcessorFilter =
    VideoProcessorFilter(6);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_STEREO_ADJUSTMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter).
pub const D3D11_VIDEO_PROCESSOR_FILTER_STEREO_ADJUSTMENT: VideoProcessorFilter =
    VideoProcessorFilter(7);

/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorFilterCaps(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS_BRIGHTNESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CAPS_BRIGHTNESS: VideoProcessorFilterCaps =
    VideoProcessorFilterCaps(1);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS_CONTRAST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CAPS_CONTRAST: VideoProcessorFilterCaps =
    VideoProcessorFilterCaps(2);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS_HUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CAPS_HUE: VideoProcessorFilterCaps =
    VideoProcessorFilterCaps(4);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS_SATURATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CAPS_SATURATION: VideoProcessorFilterCaps =
    VideoProcessorFilterCaps(8);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS_NOISE_REDUCTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CAPS_NOISE_REDUCTION: VideoProcessorFilterCaps =
    VideoProcessorFilterCaps(16);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS_EDGE_ENHANCEMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CAPS_EDGE_ENHANCEMENT: VideoProcessorFilterCaps =
    VideoProcessorFilterCaps(32);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS_ANAMORPHIC_SCALING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CAPS_ANAMORPHIC_SCALING: VideoProcessorFilterCaps =
    VideoProcessorFilterCaps(64);
/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_CAPS_STEREO_ADJUSTMENT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_filter_caps).
pub const D3D11_VIDEO_PROCESSOR_FILTER_CAPS_STEREO_ADJUSTMENT: VideoProcessorFilterCaps =
    VideoProcessorFilterCaps(128);

/// The header's `D3D11_VIDEO_PROCESSOR_FORMAT_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_format_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorFormatCaps(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_FORMAT_CAPS_RGB_INTERLACED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_format_caps).
pub const D3D11_VIDEO_PROCESSOR_FORMAT_CAPS_RGB_INTERLACED: VideoProcessorFormatCaps =
    VideoProcessorFormatCaps(1);
/// The header's `D3D11_VIDEO_PROCESSOR_FORMAT_CAPS_RGB_PROCAMP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_format_caps).
pub const D3D11_VIDEO_PROCESSOR_FORMAT_CAPS_RGB_PROCAMP: VideoProcessorFormatCaps =
    VideoProcessorFormatCaps(2);
/// The header's `D3D11_VIDEO_PROCESSOR_FORMAT_CAPS_RGB_LUMA_KEY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_format_caps).
pub const D3D11_VIDEO_PROCESSOR_FORMAT_CAPS_RGB_LUMA_KEY: VideoProcessorFormatCaps =
    VideoProcessorFormatCaps(4);
/// The header's `D3D11_VIDEO_PROCESSOR_FORMAT_CAPS_PALETTE_INTERLACED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_format_caps).
pub const D3D11_VIDEO_PROCESSOR_FORMAT_CAPS_PALETTE_INTERLACED: VideoProcessorFormatCaps =
    VideoProcessorFormatCaps(8);

/// The header's `D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_format_support).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorFormatSupport(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_format_support).
pub const D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT: VideoProcessorFormatSupport =
    VideoProcessorFormatSupport(1);
/// The header's `D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_format_support).
pub const D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT: VideoProcessorFormatSupport =
    VideoProcessorFormatSupport(2);

/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorItelecineCaps(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_32`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_32: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(1);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_22`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_22: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(2);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_2224`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_2224: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(4);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_2332`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_2332: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(8);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_32322`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_32322: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(16);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_55`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_55: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(32);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_64: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(64);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_87`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_87: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(128);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_222222222223`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_222222222223: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(0x100);
/// The header's `D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_OTHER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_itelecine_caps).
pub const D3D11_VIDEO_PROCESSOR_ITELECINE_CAPS_OTHER: VideoProcessorItelecineCaps =
    VideoProcessorItelecineCaps(-2147483648);

/// The header's `D3D11_VIDEO_PROCESSOR_NOMINAL_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_nominal_range).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorNominalRange(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_NOMINAL_RANGE_UNDEFINED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_nominal_range).
pub const D3D11_VIDEO_PROCESSOR_NOMINAL_RANGE_UNDEFINED: VideoProcessorNominalRange =
    VideoProcessorNominalRange(0);
/// The header's `D3D11_VIDEO_PROCESSOR_NOMINAL_RANGE_16_235`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_nominal_range).
pub const D3D11_VIDEO_PROCESSOR_NOMINAL_RANGE_16_235: VideoProcessorNominalRange =
    VideoProcessorNominalRange(1);
/// The header's `D3D11_VIDEO_PROCESSOR_NOMINAL_RANGE_0_255`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_nominal_range).
pub const D3D11_VIDEO_PROCESSOR_NOMINAL_RANGE_0_255: VideoProcessorNominalRange =
    VideoProcessorNominalRange(2);

/// The header's `D3D11_VIDEO_PROCESSOR_OUTPUT_RATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_output_rate).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorOutputRate(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_OUTPUT_RATE_NORMAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_output_rate).
pub const D3D11_VIDEO_PROCESSOR_OUTPUT_RATE_NORMAL: VideoProcessorOutputRate =
    VideoProcessorOutputRate(0);
/// The header's `D3D11_VIDEO_PROCESSOR_OUTPUT_RATE_HALF`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_output_rate).
pub const D3D11_VIDEO_PROCESSOR_OUTPUT_RATE_HALF: VideoProcessorOutputRate =
    VideoProcessorOutputRate(1);
/// The header's `D3D11_VIDEO_PROCESSOR_OUTPUT_RATE_CUSTOM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_output_rate).
pub const D3D11_VIDEO_PROCESSOR_OUTPUT_RATE_CUSTOM: VideoProcessorOutputRate =
    VideoProcessorOutputRate(2);

/// The header's `D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_processor_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorProcessorCaps(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_DEINTERLACE_BLEND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_processor_caps).
pub const D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_DEINTERLACE_BLEND: VideoProcessorProcessorCaps =
    VideoProcessorProcessorCaps(1);
/// The header's `D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_DEINTERLACE_BOB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_processor_caps).
pub const D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_DEINTERLACE_BOB: VideoProcessorProcessorCaps =
    VideoProcessorProcessorCaps(2);
/// The header's `D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_DEINTERLACE_ADAPTIVE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_processor_caps).
pub const D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_DEINTERLACE_ADAPTIVE: VideoProcessorProcessorCaps =
    VideoProcessorProcessorCaps(4);
/// The header's `D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_DEINTERLACE_MOTION_COMPENSATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_processor_caps).
pub const D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_DEINTERLACE_MOTION_COMPENSATION:
    VideoProcessorProcessorCaps = VideoProcessorProcessorCaps(8);
/// The header's `D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_INVERSE_TELECINE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_processor_caps).
pub const D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_INVERSE_TELECINE: VideoProcessorProcessorCaps =
    VideoProcessorProcessorCaps(16);
/// The header's `D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_FRAME_RATE_CONVERSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_processor_caps).
pub const D3D11_VIDEO_PROCESSOR_PROCESSOR_CAPS_FRAME_RATE_CONVERSION: VideoProcessorProcessorCaps =
    VideoProcessorProcessorCaps(32);

/// The header's `D3D11_VIDEO_PROCESSOR_ROTATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_rotation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorRotation(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_ROTATION_IDENTITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_rotation).
pub const D3D11_VIDEO_PROCESSOR_ROTATION_IDENTITY: VideoProcessorRotation =
    VideoProcessorRotation(0);
/// The header's `D3D11_VIDEO_PROCESSOR_ROTATION_90`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_rotation).
pub const D3D11_VIDEO_PROCESSOR_ROTATION_90: VideoProcessorRotation = VideoProcessorRotation(1);
/// The header's `D3D11_VIDEO_PROCESSOR_ROTATION_180`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_rotation).
pub const D3D11_VIDEO_PROCESSOR_ROTATION_180: VideoProcessorRotation = VideoProcessorRotation(2);
/// The header's `D3D11_VIDEO_PROCESSOR_ROTATION_270`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_rotation).
pub const D3D11_VIDEO_PROCESSOR_ROTATION_270: VideoProcessorRotation = VideoProcessorRotation(3);

/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_caps).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorStereoCaps(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_CAPS_MONO_OFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_caps).
pub const D3D11_VIDEO_PROCESSOR_STEREO_CAPS_MONO_OFFSET: VideoProcessorStereoCaps =
    VideoProcessorStereoCaps(1);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_CAPS_ROW_INTERLEAVED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_caps).
pub const D3D11_VIDEO_PROCESSOR_STEREO_CAPS_ROW_INTERLEAVED: VideoProcessorStereoCaps =
    VideoProcessorStereoCaps(2);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_CAPS_COLUMN_INTERLEAVED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_caps).
pub const D3D11_VIDEO_PROCESSOR_STEREO_CAPS_COLUMN_INTERLEAVED: VideoProcessorStereoCaps =
    VideoProcessorStereoCaps(4);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_CAPS_CHECKERBOARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_caps).
pub const D3D11_VIDEO_PROCESSOR_STEREO_CAPS_CHECKERBOARD: VideoProcessorStereoCaps =
    VideoProcessorStereoCaps(8);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_CAPS_FLIP_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_caps).
pub const D3D11_VIDEO_PROCESSOR_STEREO_CAPS_FLIP_MODE: VideoProcessorStereoCaps =
    VideoProcessorStereoCaps(16);

/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FLIP_MODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_flip_mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorStereoFlipMode(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FLIP_NONE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_flip_mode).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FLIP_NONE: VideoProcessorStereoFlipMode =
    VideoProcessorStereoFlipMode(0);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FLIP_FRAME0`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_flip_mode).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FLIP_FRAME0: VideoProcessorStereoFlipMode =
    VideoProcessorStereoFlipMode(1);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FLIP_FRAME1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_flip_mode).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FLIP_FRAME1: VideoProcessorStereoFlipMode =
    VideoProcessorStereoFlipMode(2);

/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoProcessorStereoFormat(pub i32);

/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_MONO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_MONO: VideoProcessorStereoFormat =
    VideoProcessorStereoFormat(0);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_HORIZONTAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_HORIZONTAL: VideoProcessorStereoFormat =
    VideoProcessorStereoFormat(1);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_VERTICAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_VERTICAL: VideoProcessorStereoFormat =
    VideoProcessorStereoFormat(2);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_SEPARATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_SEPARATE: VideoProcessorStereoFormat =
    VideoProcessorStereoFormat(3);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_MONO_OFFSET`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_MONO_OFFSET: VideoProcessorStereoFormat =
    VideoProcessorStereoFormat(4);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_ROW_INTERLEAVED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_ROW_INTERLEAVED: VideoProcessorStereoFormat =
    VideoProcessorStereoFormat(5);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_COLUMN_INTERLEAVED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_COLUMN_INTERLEAVED: VideoProcessorStereoFormat =
    VideoProcessorStereoFormat(6);
/// The header's `D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_CHECKERBOARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_processor_stereo_format).
pub const D3D11_VIDEO_PROCESSOR_STEREO_FORMAT_CHECKERBOARD: VideoProcessorStereoFormat =
    VideoProcessorStereoFormat(7);

/// The header's `D3D11_VIDEO_USAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_usage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VideoUsage(pub i32);

/// The header's `D3D11_VIDEO_USAGE_PLAYBACK_NORMAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_usage).
pub const D3D11_VIDEO_USAGE_PLAYBACK_NORMAL: VideoUsage = VideoUsage(0);
/// The header's `D3D11_VIDEO_USAGE_OPTIMAL_SPEED`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_usage).
pub const D3D11_VIDEO_USAGE_OPTIMAL_SPEED: VideoUsage = VideoUsage(1);
/// The header's `D3D11_VIDEO_USAGE_OPTIMAL_QUALITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_video_usage).
pub const D3D11_VIDEO_USAGE_OPTIMAL_QUALITY: VideoUsage = VideoUsage(2);

/// The header's `D3D11_VPIV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vpiv_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VpivDimension(pub i32);

/// The header's `D3D11_VPIV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vpiv_dimension).
pub const D3D11_VPIV_DIMENSION_UNKNOWN: VpivDimension = VpivDimension(0);
/// The header's `D3D11_VPIV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vpiv_dimension).
pub const D3D11_VPIV_DIMENSION_TEXTURE2D: VpivDimension = VpivDimension(1);

/// The header's `D3D11_VPOV_DIMENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vpov_dimension).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VpovDimension(pub i32);

/// The header's `D3D11_VPOV_DIMENSION_UNKNOWN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vpov_dimension).
pub const D3D11_VPOV_DIMENSION_UNKNOWN: VpovDimension = VpovDimension(0);
/// The header's `D3D11_VPOV_DIMENSION_TEXTURE2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vpov_dimension).
pub const D3D11_VPOV_DIMENSION_TEXTURE2D: VpovDimension = VpovDimension(1);
/// The header's `D3D11_VPOV_DIMENSION_TEXTURE2DARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_vpov_dimension).
pub const D3D11_VPOV_DIMENSION_TEXTURE2DARRAY: VpovDimension = VpovDimension(2);

/// The header's `D3DX11_FFT_CREATE_FLAG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_create_flag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct D3Dx11FftCreateFlag(pub i32);

/// The header's `D3DX11_FFT_CREATE_FLAG_NO_PRECOMPUTE_BUFFERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_create_flag).
pub const D3DX11_FFT_CREATE_FLAG_NO_PRECOMPUTE_BUFFERS: D3Dx11FftCreateFlag =
    D3Dx11FftCreateFlag(1);

/// The header's `D3DX11_FFT_DATA_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_data_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct D3Dx11FftDataType(pub i32);

/// The header's `D3DX11_FFT_DATA_TYPE_REAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_data_type).
pub const D3DX11_FFT_DATA_TYPE_REAL: D3Dx11FftDataType = D3Dx11FftDataType(0);
/// The header's `D3DX11_FFT_DATA_TYPE_COMPLEX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_data_type).
pub const D3DX11_FFT_DATA_TYPE_COMPLEX: D3Dx11FftDataType = D3Dx11FftDataType(1);

/// The header's `D3DX11_FFT_DIM_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_dim_mask).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct D3Dx11FftDimMask(pub i32);

/// The header's `D3DX11_FFT_DIM_MASK_1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_dim_mask).
pub const D3DX11_FFT_DIM_MASK_1D: D3Dx11FftDimMask = D3Dx11FftDimMask(1);
/// The header's `D3DX11_FFT_DIM_MASK_2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_dim_mask).
pub const D3DX11_FFT_DIM_MASK_2D: D3Dx11FftDimMask = D3Dx11FftDimMask(3);
/// The header's `D3DX11_FFT_DIM_MASK_3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_fft_dim_mask).
pub const D3DX11_FFT_DIM_MASK_3D: D3Dx11FftDimMask = D3Dx11FftDimMask(7);

/// The header's `D3DX11_SCAN_DATA_TYPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_data_type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct D3Dx11ScanDataType(pub i32);

/// The header's `D3DX11_SCAN_DATA_TYPE_FLOAT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_data_type).
pub const D3DX11_SCAN_DATA_TYPE_FLOAT: D3Dx11ScanDataType = D3Dx11ScanDataType(1);
/// The header's `D3DX11_SCAN_DATA_TYPE_INT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_data_type).
pub const D3DX11_SCAN_DATA_TYPE_INT: D3Dx11ScanDataType = D3Dx11ScanDataType(2);
/// The header's `D3DX11_SCAN_DATA_TYPE_UINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_data_type).
pub const D3DX11_SCAN_DATA_TYPE_UINT: D3Dx11ScanDataType = D3Dx11ScanDataType(3);

/// The header's `D3DX11_SCAN_DIRECTION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_direction).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct D3Dx11ScanDirection(pub i32);

/// The header's `D3DX11_SCAN_DIRECTION_FORWARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_direction).
pub const D3DX11_SCAN_DIRECTION_FORWARD: D3Dx11ScanDirection = D3Dx11ScanDirection(1);
/// The header's `D3DX11_SCAN_DIRECTION_BACKWARD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_direction).
pub const D3DX11_SCAN_DIRECTION_BACKWARD: D3Dx11ScanDirection = D3Dx11ScanDirection(2);

/// The header's `D3DX11_SCAN_OPCODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_opcode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct D3Dx11ScanOpcode(pub i32);

/// The header's `D3DX11_SCAN_OPCODE_ADD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_opcode).
pub const D3DX11_SCAN_OPCODE_ADD: D3Dx11ScanOpcode = D3Dx11ScanOpcode(1);
/// The header's `D3DX11_SCAN_OPCODE_MIN`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_opcode).
pub const D3DX11_SCAN_OPCODE_MIN: D3Dx11ScanOpcode = D3Dx11ScanOpcode(2);
/// The header's `D3DX11_SCAN_OPCODE_MAX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_opcode).
pub const D3DX11_SCAN_OPCODE_MAX: D3Dx11ScanOpcode = D3Dx11ScanOpcode(3);
/// The header's `D3DX11_SCAN_OPCODE_MUL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_opcode).
pub const D3DX11_SCAN_OPCODE_MUL: D3Dx11ScanOpcode = D3Dx11ScanOpcode(4);
/// The header's `D3DX11_SCAN_OPCODE_AND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_opcode).
pub const D3DX11_SCAN_OPCODE_AND: D3Dx11ScanOpcode = D3Dx11ScanOpcode(5);
/// The header's `D3DX11_SCAN_OPCODE_OR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_opcode).
pub const D3DX11_SCAN_OPCODE_OR: D3Dx11ScanOpcode = D3Dx11ScanOpcode(6);
/// The header's `D3DX11_SCAN_OPCODE_XOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ne-d3dcsx-d3dx11_scan_opcode).
pub const D3DX11_SCAN_OPCODE_XOR: D3Dx11ScanOpcode = D3Dx11ScanOpcode(7);
