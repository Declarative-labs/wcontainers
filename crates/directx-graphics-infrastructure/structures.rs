//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

use wcontainers_component_object_model as component_object_model;

/// The header's `DXGI_ADAPTER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_adapter_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdapterDesc {
    pub description: [u16; crate::constants::DXGI_ADAPTER_DESC_DESCRIPTION_LENGTH],
    pub vendor_id: u32,
    pub device_id: u32,
    pub sub_sys_id: u32,
    pub revision: u32,
    pub dedicated_video_memory: usize,
    pub dedicated_system_memory: usize,
    pub shared_system_memory: usize,
    pub adapter_luid: component_object_model::structures::Luid,
}

/// The header's `DXGI_ADAPTER_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_adapter_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdapterDesc1 {
    pub description: [u16; crate::constants::DXGI_ADAPTER_DESC1_DESCRIPTION_LENGTH],
    pub vendor_id: u32,
    pub device_id: u32,
    pub sub_sys_id: u32,
    pub revision: u32,
    pub dedicated_video_memory: usize,
    pub dedicated_system_memory: usize,
    pub shared_system_memory: usize,
    pub adapter_luid: component_object_model::structures::Luid,
    pub flags: u32,
}

/// The header's `DXGI_ADAPTER_DESC2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_adapter_desc2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdapterDesc2 {
    pub description: [u16; crate::constants::DXGI_ADAPTER_DESC2_DESCRIPTION_LENGTH],
    pub vendor_id: u32,
    pub device_id: u32,
    pub sub_sys_id: u32,
    pub revision: u32,
    pub dedicated_video_memory: usize,
    pub dedicated_system_memory: usize,
    pub shared_system_memory: usize,
    pub adapter_luid: component_object_model::structures::Luid,
    pub flags: u32,
    pub graphics_preemption_granularity: crate::enumerations::GraphicsPreemptionGranularity,
    pub compute_preemption_granularity: crate::enumerations::ComputePreemptionGranularity,
}

/// The header's `DXGI_ADAPTER_DESC3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ns-dxgi1_6-dxgi_adapter_desc3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdapterDesc3 {
    pub description: [u16; crate::constants::DXGI_ADAPTER_DESC3_DESCRIPTION_LENGTH],
    pub vendor_id: u32,
    pub device_id: u32,
    pub sub_sys_id: u32,
    pub revision: u32,
    pub dedicated_video_memory: usize,
    pub dedicated_system_memory: usize,
    pub shared_system_memory: usize,
    pub adapter_luid: component_object_model::structures::Luid,
    pub flags: crate::enumerations::AdapterFlag3,
    pub graphics_preemption_granularity: crate::enumerations::GraphicsPreemptionGranularity,
    pub compute_preemption_granularity: crate::enumerations::ComputePreemptionGranularity,
}

/// The header's `DXGI_DECODE_SWAP_CHAIN_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ns-dxgi1_3-dxgi_decode_swap_chain_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecodeSwapChainDesc {
    pub flags: u32,
}

/// The header's `DXGI_DISPLAY_COLOR_SPACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_display_color_space).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayColorSpace {
    pub primary_coordinates:
        [f32; crate::constants::DXGI_DISPLAY_COLOR_SPACE_PRIMARYCOORDINATES_LENGTH],
    pub white_points: [f32; crate::constants::DXGI_DISPLAY_COLOR_SPACE_WHITEPOINTS_LENGTH],
}

/// The header's `DXGI_FRAME_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_frame_statistics).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameStatistics {
    pub present_count: u32,
    pub present_refresh_count: u32,
    pub sync_refresh_count: u32,
    pub sync_qpc_time: i64,
    pub sync_gpu_time: i64,
}

/// The header's `DXGI_FRAME_STATISTICS_MEDIA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ns-dxgi1_3-dxgi_frame_statistics_media).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameStatisticsMedia {
    pub present_count: u32,
    pub present_refresh_count: u32,
    pub sync_refresh_count: u32,
    pub sync_qpc_time: i64,
    pub sync_gpu_time: i64,
    pub composition_mode: crate::enumerations::FramePresentationMode,
    pub approved_present_duration: u32,
}

/// The header's `DXGI_HDR_METADATA_HDR10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/ns-dxgi1_5-dxgi_hdr_metadata_hdr10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HdrMetadataHdr10 {
    pub red_primary: [u16; crate::constants::DXGI_HDR_METADATA_HDR10_REDPRIMARY_LENGTH],
    pub green_primary: [u16; crate::constants::DXGI_HDR_METADATA_HDR10_GREENPRIMARY_LENGTH],
    pub blue_primary: [u16; crate::constants::DXGI_HDR_METADATA_HDR10_BLUEPRIMARY_LENGTH],
    pub white_point: [u16; crate::constants::DXGI_HDR_METADATA_HDR10_WHITEPOINT_LENGTH],
    pub max_mastering_luminance: u32,
    pub min_mastering_luminance: u32,
    pub max_content_light_level: u16,
    pub max_frame_average_light_level: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HdrMetadataHdr10plus {
    pub data: [u8; crate::constants::DXGI_HDR_METADATA_HDR10PLUS_DATA_LENGTH],
}

/// The header's `DXGI_INFO_QUEUE_FILTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ns-dxgidebug-dxgi_info_queue_filter).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoQueueFilter {
    pub allow_list: crate::structures::InfoQueueFilterDesc,
    pub deny_list: crate::structures::InfoQueueFilterDesc,
}

/// The header's `DXGI_INFO_QUEUE_FILTER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ns-dxgidebug-dxgi_info_queue_filter_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoQueueFilterDesc {
    pub num_categories: u32,
    pub category_list: Vec<crate::enumerations::InfoQueueMessageCategory>,
    pub num_severities: u32,
    pub severity_list: Vec<crate::enumerations::InfoQueueMessageSeverity>,
    pub num_i_ds: u32,
    pub id_list: Vec<i32>,
}

/// The header's `DXGI_INFO_QUEUE_MESSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/ns-dxgidebug-dxgi_info_queue_message).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoQueueMessage {
    pub producer: component_object_model::structures::Guid,
    pub category: crate::enumerations::InfoQueueMessageCategory,
    pub severity: crate::enumerations::InfoQueueMessageSeverity,
    pub id: i32,
    pub description: Vec<u8>,
    pub description_byte_length: usize,
}

/// The header's `DXGI_MAPPED_RECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_mapped_rect).
#[derive(Clone, Debug, PartialEq)]
pub struct MappedRect {
    pub pitch: i32,
    pub bits: Vec<u8>,
}

/// The header's `DXGI_MATRIX_3X2_F`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/ns-dxgi1_3-dxgi_matrix_3x2_f).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix3x2F {
    pub _11: f32,
    pub _12: f32,
    pub _21: f32,
    pub _22: f32,
    pub _31: f32,
    pub _32: f32,
}

/// The header's `DXGI_MODE_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_mode_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeDesc1 {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: crate::structures::Rational,
    pub format: crate::enumerations::Format,
    pub scanline_ordering: crate::enumerations::ModeScanlineOrder,
    pub scaling: crate::enumerations::ModeScaling,
    pub stereo: bool,
}

/// The header's `DXGI_OUTDUPL_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutduplDesc {
    pub mode_desc: crate::structures::ModeDesc,
    pub rotation: crate::enumerations::ModeRotation,
    pub desktop_image_in_system_memory: bool,
}

/// The header's `DXGI_OUTDUPL_FRAME_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutduplFrameInfo {
    pub last_present_time: i64,
    pub last_mouse_update_time: i64,
    pub accumulated_frames: u32,
    pub rects_coalesced: bool,
    pub protected_content_masked_out: bool,
    pub pointer_position: crate::structures::OutduplPointerPosition,
    pub total_metadata_buffer_size: u32,
    pub pointer_shape_buffer_size: u32,
}

/// The header's `DXGI_OUTDUPL_MOVE_RECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_move_rect).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutduplMoveRect {
    pub source_point: component_object_model::structures::Point,
    pub destination_rect: component_object_model::structures::Rect,
}

/// The header's `DXGI_OUTDUPL_POINTER_POSITION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_pointer_position).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutduplPointerPosition {
    pub position: component_object_model::structures::Point,
    pub visible: bool,
}

/// The header's `DXGI_OUTDUPL_POINTER_SHAPE_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_pointer_shape_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutduplPointerShapeInfo {
    pub type_: u32,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub hot_spot: component_object_model::structures::Point,
}

/// The header's `DXGI_OUTPUT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_output_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputDesc {
    pub device_name: [u16; crate::constants::DXGI_OUTPUT_DESC_DEVICENAME_LENGTH],
    pub desktop_coordinates: component_object_model::structures::Rect,
    pub attached_to_desktop: bool,
    pub rotation: crate::enumerations::ModeRotation,
    pub monitor: component_object_model::structures::Hmonitor,
}

/// The header's `DXGI_OUTPUT_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/ns-dxgi1_6-dxgi_output_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputDesc1 {
    pub device_name: [u16; crate::constants::DXGI_OUTPUT_DESC1_DEVICENAME_LENGTH],
    pub desktop_coordinates: component_object_model::structures::Rect,
    pub attached_to_desktop: bool,
    pub rotation: crate::enumerations::ModeRotation,
    pub monitor: component_object_model::structures::Hmonitor,
    pub bits_per_color: u32,
    pub color_space: crate::enumerations::ColorSpaceType,
    pub red_primary: [f32; crate::constants::DXGI_OUTPUT_DESC1_REDPRIMARY_LENGTH],
    pub green_primary: [f32; crate::constants::DXGI_OUTPUT_DESC1_GREENPRIMARY_LENGTH],
    pub blue_primary: [f32; crate::constants::DXGI_OUTPUT_DESC1_BLUEPRIMARY_LENGTH],
    pub white_point: [f32; crate::constants::DXGI_OUTPUT_DESC1_WHITEPOINT_LENGTH],
    pub min_luminance: f32,
    pub max_luminance: f32,
    pub max_full_frame_luminance: f32,
}

/// The header's `DXGI_PRESENT_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_present_parameters).
#[derive(Clone, Debug, PartialEq)]
pub struct PresentParameters {
    pub dirty_rects_count: u32,
    pub dirty_rects: Vec<component_object_model::structures::Rect>,
    pub scroll_rect: Vec<component_object_model::structures::Rect>,
    pub scroll_offset: Vec<component_object_model::structures::Point>,
}

/// The header's `DXGI_QUERY_VIDEO_MEMORY_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/ns-dxgi1_4-dxgi_query_video_memory_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryVideoMemoryInfo {
    pub budget: u64,
    pub current_usage: u64,
    pub available_for_reservation: u64,
    pub current_reservation: u64,
}

/// The header's `DXGI_RGBA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-rgba).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// The header's `DXGI_SHARED_RESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_shared_resource).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SharedResource {
    pub handle: component_object_model::structures::Handle,
}

/// The header's `DXGI_SURFACE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_surface_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceDesc {
    pub width: u32,
    pub height: u32,
    pub format: crate::enumerations::Format,
    pub sample_desc: crate::structures::SampleDesc,
}

/// The header's `DXGI_SWAP_CHAIN_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/ns-dxgi-dxgi_swap_chain_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwapChainDesc {
    pub buffer_desc: crate::structures::ModeDesc,
    pub sample_desc: crate::structures::SampleDesc,
    pub buffer_usage: crate::enumerations::Usage,
    pub buffer_count: u32,
    pub output_window: component_object_model::structures::Hwnd,
    pub windowed: bool,
    pub swap_effect: crate::enumerations::SwapEffect,
    pub flags: u32,
}

/// The header's `DXGI_SWAP_CHAIN_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_swap_chain_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwapChainDesc1 {
    pub width: u32,
    pub height: u32,
    pub format: crate::enumerations::Format,
    pub stereo: bool,
    pub sample_desc: crate::structures::SampleDesc,
    pub buffer_usage: crate::enumerations::Usage,
    pub buffer_count: u32,
    pub scaling: crate::enumerations::Scaling,
    pub swap_effect: crate::enumerations::SwapEffect,
    pub alpha_mode: crate::enumerations::AlphaMode,
    pub flags: u32,
}

/// The header's `DXGI_SWAP_CHAIN_FULLSCREEN_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_swap_chain_fullscreen_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwapChainFullscreenDesc {
    pub refresh_rate: crate::structures::Rational,
    pub scanline_ordering: crate::enumerations::ModeScanlineOrder,
    pub scaling: crate::enumerations::ModeScaling,
    pub windowed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GammaControl {
    pub scale: crate::structures::Rgb,
    pub offset: crate::structures::Rgb,
    pub gamma_curve:
        [crate::structures::Rgb; crate::constants::DXGI_GAMMA_CONTROL_GAMMACURVE_LENGTH],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GammaControlCapabilities {
    pub scale_and_offset_supported: bool,
    pub max_converted_value: f32,
    pub min_converted_value: f32,
    pub num_gamma_control_points: u32,
    pub control_point_positions:
        [f32; crate::constants::DXGI_GAMMA_CONTROL_CAPABILITIES_CONTROLPOINTPOSITIONS_LENGTH],
}

/// The header's `DXGI_JPEG_AC_HUFFMAN_TABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-jpeg-ac-huffman-table).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JpegAcHuffmanTable {
    pub code_counts: [u8; crate::constants::DXGI_JPEG_AC_HUFFMAN_TABLE_CODECOUNTS_LENGTH],
    pub code_values: [u8; crate::constants::DXGI_JPEG_AC_HUFFMAN_TABLE_CODEVALUES_LENGTH],
}

/// The header's `DXGI_JPEG_DC_HUFFMAN_TABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-jpeg-dc-huffman-table).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JpegDcHuffmanTable {
    pub code_counts: [u8; crate::constants::DXGI_JPEG_DC_HUFFMAN_TABLE_CODECOUNTS_LENGTH],
    pub code_values: [u8; crate::constants::DXGI_JPEG_DC_HUFFMAN_TABLE_CODEVALUES_LENGTH],
}

/// The header's `DXGI_JPEG_QUANTIZATION_TABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-jpeg-quantization-table).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JpegQuantizationTable {
    pub elements: [u8; crate::constants::DXGI_JPEG_QUANTIZATION_TABLE_ELEMENTS_LENGTH],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeDesc {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: crate::structures::Rational,
    pub format: crate::enumerations::Format,
    pub scanline_ordering: crate::enumerations::ModeScanlineOrder,
    pub scaling: crate::enumerations::ModeScaling,
}

/// The header's `DXGI_RATIONAL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ns-dxgicommon-dxgi_rational).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rational {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

/// The header's `DXGI_SAMPLE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgicommon/ns-dxgicommon-dxgi_sample_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleDesc {
    pub count: u32,
    pub quality: u32,
}
