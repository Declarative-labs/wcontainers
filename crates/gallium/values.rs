// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers (p_defines.h, p_state.h, p_screen.h, p_context.h, u_transfer_helper.h; blend.h, shader_enums.h, u_formats.h and the build's u_format_gen.h for the values) and the plan in src/gallium.rs. Do not edit.
//! Gallium's declared values, under their C names: each enumeration of Mesa's headers a type holding its
//! enumerators, and the anonymous enumerations' enumerators and the `PIPE_*` defines as constants.

/// Mesa's `mesa_shader_stage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MesaShaderStage(pub i32);

impl MesaShaderStage {
    /// `MESA_SHADER_NONE`.
    pub const MESA_SHADER_NONE: MesaShaderStage = MesaShaderStage(-1);
    /// `MESA_SHADER_VERTEX`.
    pub const MESA_SHADER_VERTEX: MesaShaderStage = MesaShaderStage(0);
    /// `MESA_SHADER_TESS_CTRL`.
    pub const MESA_SHADER_TESS_CTRL: MesaShaderStage = MesaShaderStage(1);
    /// `MESA_SHADER_TESS_EVAL`.
    pub const MESA_SHADER_TESS_EVAL: MesaShaderStage = MesaShaderStage(2);
    /// `MESA_SHADER_GEOMETRY`.
    pub const MESA_SHADER_GEOMETRY: MesaShaderStage = MesaShaderStage(3);
    /// `MESA_SHADER_FRAGMENT`.
    pub const MESA_SHADER_FRAGMENT: MesaShaderStage = MesaShaderStage(4);
    /// `MESA_SHADER_COMPUTE`.
    pub const MESA_SHADER_COMPUTE: MesaShaderStage = MesaShaderStage(5);
    /// `MESA_SHADER_TASK`.
    pub const MESA_SHADER_TASK: MesaShaderStage = MesaShaderStage(6);
    /// `MESA_SHADER_MESH`.
    pub const MESA_SHADER_MESH: MesaShaderStage = MesaShaderStage(7);
    /// `MESA_SHADER_RAYGEN`.
    pub const MESA_SHADER_RAYGEN: MesaShaderStage = MesaShaderStage(8);
    /// `MESA_SHADER_ANY_HIT`.
    pub const MESA_SHADER_ANY_HIT: MesaShaderStage = MesaShaderStage(9);
    /// `MESA_SHADER_CLOSEST_HIT`.
    pub const MESA_SHADER_CLOSEST_HIT: MesaShaderStage = MesaShaderStage(10);
    /// `MESA_SHADER_MISS`.
    pub const MESA_SHADER_MISS: MesaShaderStage = MesaShaderStage(11);
    /// `MESA_SHADER_INTERSECTION`.
    pub const MESA_SHADER_INTERSECTION: MesaShaderStage = MesaShaderStage(12);
    /// `MESA_SHADER_CALLABLE`.
    pub const MESA_SHADER_CALLABLE: MesaShaderStage = MesaShaderStage(13);
    /// `MESA_SHADER_KERNEL`.
    pub const MESA_SHADER_KERNEL: MesaShaderStage = MesaShaderStage(14);
}

/// Mesa's `mesa_prim`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MesaPrim(pub u32);

impl MesaPrim {
    /// `MESA_PRIM_POINTS`.
    pub const MESA_PRIM_POINTS: MesaPrim = MesaPrim(0);
    /// `MESA_PRIM_LINES`.
    pub const MESA_PRIM_LINES: MesaPrim = MesaPrim(1);
    /// `MESA_PRIM_LINE_LOOP`.
    pub const MESA_PRIM_LINE_LOOP: MesaPrim = MesaPrim(2);
    /// `MESA_PRIM_LINE_STRIP`.
    pub const MESA_PRIM_LINE_STRIP: MesaPrim = MesaPrim(3);
    /// `MESA_PRIM_TRIANGLES`.
    pub const MESA_PRIM_TRIANGLES: MesaPrim = MesaPrim(4);
    /// `MESA_PRIM_TRIANGLE_STRIP`.
    pub const MESA_PRIM_TRIANGLE_STRIP: MesaPrim = MesaPrim(5);
    /// `MESA_PRIM_TRIANGLE_FAN`.
    pub const MESA_PRIM_TRIANGLE_FAN: MesaPrim = MesaPrim(6);
    /// `MESA_PRIM_QUADS`.
    pub const MESA_PRIM_QUADS: MesaPrim = MesaPrim(7);
    /// `MESA_PRIM_QUAD_STRIP`.
    pub const MESA_PRIM_QUAD_STRIP: MesaPrim = MesaPrim(8);
    /// `MESA_PRIM_POLYGON`.
    pub const MESA_PRIM_POLYGON: MesaPrim = MesaPrim(9);
    /// `MESA_PRIM_LINES_ADJACENCY`.
    pub const MESA_PRIM_LINES_ADJACENCY: MesaPrim = MesaPrim(10);
    /// `MESA_PRIM_LINE_STRIP_ADJACENCY`.
    pub const MESA_PRIM_LINE_STRIP_ADJACENCY: MesaPrim = MesaPrim(11);
    /// `MESA_PRIM_TRIANGLES_ADJACENCY`.
    pub const MESA_PRIM_TRIANGLES_ADJACENCY: MesaPrim = MesaPrim(12);
    /// `MESA_PRIM_TRIANGLE_STRIP_ADJACENCY`.
    pub const MESA_PRIM_TRIANGLE_STRIP_ADJACENCY: MesaPrim = MesaPrim(13);
    /// `MESA_PRIM_PATCHES`.
    pub const MESA_PRIM_PATCHES: MesaPrim = MesaPrim(14);
    /// `MESA_PRIM_MAX`.
    pub const MESA_PRIM_MAX: MesaPrim = MesaPrim(14);
    /// `MESA_PRIM_COUNT`.
    pub const MESA_PRIM_COUNT: MesaPrim = MesaPrim(15);
    /// `MESA_PRIM_UNKNOWN`.
    pub const MESA_PRIM_UNKNOWN: MesaPrim = MesaPrim(28);
}

/// Mesa's `pipe_format`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Format(pub u32);

impl Format {
    /// `PIPE_FORMAT_NONE`.
    pub const PIPE_FORMAT_NONE: Format = Format(0);
    /// `PIPE_FORMAT_R64_UINT`.
    pub const PIPE_FORMAT_R64_UINT: Format = Format(1);
    /// `PIPE_FORMAT_VERTEX_START`.
    pub const PIPE_FORMAT_VERTEX_START: Format = Format(1);
    /// `PIPE_FORMAT_R64G64_UINT`.
    pub const PIPE_FORMAT_R64G64_UINT: Format = Format(2);
    /// `PIPE_FORMAT_R64G64B64_UINT`.
    pub const PIPE_FORMAT_R64G64B64_UINT: Format = Format(3);
    /// `PIPE_FORMAT_R64G64B64A64_UINT`.
    pub const PIPE_FORMAT_R64G64B64A64_UINT: Format = Format(4);
    /// `PIPE_FORMAT_R64_SINT`.
    pub const PIPE_FORMAT_R64_SINT: Format = Format(5);
    /// `PIPE_FORMAT_R64G64_SINT`.
    pub const PIPE_FORMAT_R64G64_SINT: Format = Format(6);
    /// `PIPE_FORMAT_R64G64B64_SINT`.
    pub const PIPE_FORMAT_R64G64B64_SINT: Format = Format(7);
    /// `PIPE_FORMAT_R64G64B64A64_SINT`.
    pub const PIPE_FORMAT_R64G64B64A64_SINT: Format = Format(8);
    /// `PIPE_FORMAT_R64_FLOAT`.
    pub const PIPE_FORMAT_R64_FLOAT: Format = Format(9);
    /// `PIPE_FORMAT_R64G64_FLOAT`.
    pub const PIPE_FORMAT_R64G64_FLOAT: Format = Format(10);
    /// `PIPE_FORMAT_R64G64B64_FLOAT`.
    pub const PIPE_FORMAT_R64G64B64_FLOAT: Format = Format(11);
    /// `PIPE_FORMAT_R64G64B64A64_FLOAT`.
    pub const PIPE_FORMAT_R64G64B64A64_FLOAT: Format = Format(12);
    /// `PIPE_FORMAT_R32_UINT`.
    pub const PIPE_FORMAT_R32_UINT: Format = Format(13);
    /// `PIPE_FORMAT_R32G32_UINT`.
    pub const PIPE_FORMAT_R32G32_UINT: Format = Format(14);
    /// `PIPE_FORMAT_R32G32B32_UINT`.
    pub const PIPE_FORMAT_R32G32B32_UINT: Format = Format(15);
    /// `PIPE_FORMAT_R32G32B32A32_UINT`.
    pub const PIPE_FORMAT_R32G32B32A32_UINT: Format = Format(16);
    /// `PIPE_FORMAT_R32_SINT`.
    pub const PIPE_FORMAT_R32_SINT: Format = Format(17);
    /// `PIPE_FORMAT_R32G32_SINT`.
    pub const PIPE_FORMAT_R32G32_SINT: Format = Format(18);
    /// `PIPE_FORMAT_R32G32B32_SINT`.
    pub const PIPE_FORMAT_R32G32B32_SINT: Format = Format(19);
    /// `PIPE_FORMAT_R32G32B32A32_SINT`.
    pub const PIPE_FORMAT_R32G32B32A32_SINT: Format = Format(20);
    /// `PIPE_FORMAT_R32_FLOAT`.
    pub const PIPE_FORMAT_R32_FLOAT: Format = Format(21);
    /// `PIPE_FORMAT_R32G32_FLOAT`.
    pub const PIPE_FORMAT_R32G32_FLOAT: Format = Format(22);
    /// `PIPE_FORMAT_R32G32B32_FLOAT`.
    pub const PIPE_FORMAT_R32G32B32_FLOAT: Format = Format(23);
    /// `PIPE_FORMAT_R32G32B32A32_FLOAT`.
    pub const PIPE_FORMAT_R32G32B32A32_FLOAT: Format = Format(24);
    /// `PIPE_FORMAT_R32_UNORM`.
    pub const PIPE_FORMAT_R32_UNORM: Format = Format(25);
    /// `PIPE_FORMAT_R32G32_UNORM`.
    pub const PIPE_FORMAT_R32G32_UNORM: Format = Format(26);
    /// `PIPE_FORMAT_R32G32B32_UNORM`.
    pub const PIPE_FORMAT_R32G32B32_UNORM: Format = Format(27);
    /// `PIPE_FORMAT_R32G32B32A32_UNORM`.
    pub const PIPE_FORMAT_R32G32B32A32_UNORM: Format = Format(28);
    /// `PIPE_FORMAT_R32_SNORM`.
    pub const PIPE_FORMAT_R32_SNORM: Format = Format(29);
    /// `PIPE_FORMAT_R32G32_SNORM`.
    pub const PIPE_FORMAT_R32G32_SNORM: Format = Format(30);
    /// `PIPE_FORMAT_R32G32B32_SNORM`.
    pub const PIPE_FORMAT_R32G32B32_SNORM: Format = Format(31);
    /// `PIPE_FORMAT_R32G32B32A32_SNORM`.
    pub const PIPE_FORMAT_R32G32B32A32_SNORM: Format = Format(32);
    /// `PIPE_FORMAT_R32_USCALED`.
    pub const PIPE_FORMAT_R32_USCALED: Format = Format(33);
    /// `PIPE_FORMAT_R32G32_USCALED`.
    pub const PIPE_FORMAT_R32G32_USCALED: Format = Format(34);
    /// `PIPE_FORMAT_R32G32B32_USCALED`.
    pub const PIPE_FORMAT_R32G32B32_USCALED: Format = Format(35);
    /// `PIPE_FORMAT_R32G32B32A32_USCALED`.
    pub const PIPE_FORMAT_R32G32B32A32_USCALED: Format = Format(36);
    /// `PIPE_FORMAT_R32_SSCALED`.
    pub const PIPE_FORMAT_R32_SSCALED: Format = Format(37);
    /// `PIPE_FORMAT_R32G32_SSCALED`.
    pub const PIPE_FORMAT_R32G32_SSCALED: Format = Format(38);
    /// `PIPE_FORMAT_R32G32B32_SSCALED`.
    pub const PIPE_FORMAT_R32G32B32_SSCALED: Format = Format(39);
    /// `PIPE_FORMAT_R32G32B32A32_SSCALED`.
    pub const PIPE_FORMAT_R32G32B32A32_SSCALED: Format = Format(40);
    /// `PIPE_FORMAT_R32_FIXED`.
    pub const PIPE_FORMAT_R32_FIXED: Format = Format(41);
    /// `PIPE_FORMAT_R32G32_FIXED`.
    pub const PIPE_FORMAT_R32G32_FIXED: Format = Format(42);
    /// `PIPE_FORMAT_R32G32B32_FIXED`.
    pub const PIPE_FORMAT_R32G32B32_FIXED: Format = Format(43);
    /// `PIPE_FORMAT_R32G32B32A32_FIXED`.
    pub const PIPE_FORMAT_R32G32B32A32_FIXED: Format = Format(44);
    /// `PIPE_FORMAT_R16_UINT`.
    pub const PIPE_FORMAT_R16_UINT: Format = Format(45);
    /// `PIPE_FORMAT_R16G16_UINT`.
    pub const PIPE_FORMAT_R16G16_UINT: Format = Format(46);
    /// `PIPE_FORMAT_R16G16B16_UINT`.
    pub const PIPE_FORMAT_R16G16B16_UINT: Format = Format(47);
    /// `PIPE_FORMAT_R16G16B16A16_UINT`.
    pub const PIPE_FORMAT_R16G16B16A16_UINT: Format = Format(48);
    /// `PIPE_FORMAT_R16_SINT`.
    pub const PIPE_FORMAT_R16_SINT: Format = Format(49);
    /// `PIPE_FORMAT_R16G16_SINT`.
    pub const PIPE_FORMAT_R16G16_SINT: Format = Format(50);
    /// `PIPE_FORMAT_R16G16B16_SINT`.
    pub const PIPE_FORMAT_R16G16B16_SINT: Format = Format(51);
    /// `PIPE_FORMAT_R16G16B16A16_SINT`.
    pub const PIPE_FORMAT_R16G16B16A16_SINT: Format = Format(52);
    /// `PIPE_FORMAT_R16_FLOAT`.
    pub const PIPE_FORMAT_R16_FLOAT: Format = Format(53);
    /// `PIPE_FORMAT_R16G16_FLOAT`.
    pub const PIPE_FORMAT_R16G16_FLOAT: Format = Format(54);
    /// `PIPE_FORMAT_R16G16B16_FLOAT`.
    pub const PIPE_FORMAT_R16G16B16_FLOAT: Format = Format(55);
    /// `PIPE_FORMAT_R16G16B16A16_FLOAT`.
    pub const PIPE_FORMAT_R16G16B16A16_FLOAT: Format = Format(56);
    /// `PIPE_FORMAT_R16_UNORM`.
    pub const PIPE_FORMAT_R16_UNORM: Format = Format(57);
    /// `PIPE_FORMAT_R16G16_UNORM`.
    pub const PIPE_FORMAT_R16G16_UNORM: Format = Format(58);
    /// `PIPE_FORMAT_R16G16B16_UNORM`.
    pub const PIPE_FORMAT_R16G16B16_UNORM: Format = Format(59);
    /// `PIPE_FORMAT_R16G16B16A16_UNORM`.
    pub const PIPE_FORMAT_R16G16B16A16_UNORM: Format = Format(60);
    /// `PIPE_FORMAT_R16_SNORM`.
    pub const PIPE_FORMAT_R16_SNORM: Format = Format(61);
    /// `PIPE_FORMAT_R16G16_SNORM`.
    pub const PIPE_FORMAT_R16G16_SNORM: Format = Format(62);
    /// `PIPE_FORMAT_R16G16B16_SNORM`.
    pub const PIPE_FORMAT_R16G16B16_SNORM: Format = Format(63);
    /// `PIPE_FORMAT_R16G16B16A16_SNORM`.
    pub const PIPE_FORMAT_R16G16B16A16_SNORM: Format = Format(64);
    /// `PIPE_FORMAT_R16_USCALED`.
    pub const PIPE_FORMAT_R16_USCALED: Format = Format(65);
    /// `PIPE_FORMAT_R16G16_USCALED`.
    pub const PIPE_FORMAT_R16G16_USCALED: Format = Format(66);
    /// `PIPE_FORMAT_R16G16B16_USCALED`.
    pub const PIPE_FORMAT_R16G16B16_USCALED: Format = Format(67);
    /// `PIPE_FORMAT_R16G16B16A16_USCALED`.
    pub const PIPE_FORMAT_R16G16B16A16_USCALED: Format = Format(68);
    /// `PIPE_FORMAT_R16_SSCALED`.
    pub const PIPE_FORMAT_R16_SSCALED: Format = Format(69);
    /// `PIPE_FORMAT_R16G16_SSCALED`.
    pub const PIPE_FORMAT_R16G16_SSCALED: Format = Format(70);
    /// `PIPE_FORMAT_R16G16B16_SSCALED`.
    pub const PIPE_FORMAT_R16G16B16_SSCALED: Format = Format(71);
    /// `PIPE_FORMAT_R16G16B16A16_SSCALED`.
    pub const PIPE_FORMAT_R16G16B16A16_SSCALED: Format = Format(72);
    /// `PIPE_FORMAT_R8_UINT`.
    pub const PIPE_FORMAT_R8_UINT: Format = Format(73);
    /// `PIPE_FORMAT_R8G8_UINT`.
    pub const PIPE_FORMAT_R8G8_UINT: Format = Format(74);
    /// `PIPE_FORMAT_R8G8B8_UINT`.
    pub const PIPE_FORMAT_R8G8B8_UINT: Format = Format(75);
    /// `PIPE_FORMAT_R8G8B8A8_UINT`.
    pub const PIPE_FORMAT_R8G8B8A8_UINT: Format = Format(76);
    /// `PIPE_FORMAT_R8_SINT`.
    pub const PIPE_FORMAT_R8_SINT: Format = Format(77);
    /// `PIPE_FORMAT_R8G8_SINT`.
    pub const PIPE_FORMAT_R8G8_SINT: Format = Format(78);
    /// `PIPE_FORMAT_R8G8B8_SINT`.
    pub const PIPE_FORMAT_R8G8B8_SINT: Format = Format(79);
    /// `PIPE_FORMAT_R8G8B8A8_SINT`.
    pub const PIPE_FORMAT_R8G8B8A8_SINT: Format = Format(80);
    /// `PIPE_FORMAT_R8_UNORM`.
    pub const PIPE_FORMAT_R8_UNORM: Format = Format(81);
    /// `PIPE_FORMAT_R8G8_UNORM`.
    pub const PIPE_FORMAT_R8G8_UNORM: Format = Format(82);
    /// `PIPE_FORMAT_R8G8B8_UNORM`.
    pub const PIPE_FORMAT_R8G8B8_UNORM: Format = Format(83);
    /// `PIPE_FORMAT_R8G8B8A8_UNORM`.
    pub const PIPE_FORMAT_R8G8B8A8_UNORM: Format = Format(84);
    /// `PIPE_FORMAT_R8_SNORM`.
    pub const PIPE_FORMAT_R8_SNORM: Format = Format(85);
    /// `PIPE_FORMAT_R8G8_SNORM`.
    pub const PIPE_FORMAT_R8G8_SNORM: Format = Format(86);
    /// `PIPE_FORMAT_R8G8B8_SNORM`.
    pub const PIPE_FORMAT_R8G8B8_SNORM: Format = Format(87);
    /// `PIPE_FORMAT_R8G8B8A8_SNORM`.
    pub const PIPE_FORMAT_R8G8B8A8_SNORM: Format = Format(88);
    /// `PIPE_FORMAT_R8_USCALED`.
    pub const PIPE_FORMAT_R8_USCALED: Format = Format(89);
    /// `PIPE_FORMAT_R8G8_USCALED`.
    pub const PIPE_FORMAT_R8G8_USCALED: Format = Format(90);
    /// `PIPE_FORMAT_R8G8B8_USCALED`.
    pub const PIPE_FORMAT_R8G8B8_USCALED: Format = Format(91);
    /// `PIPE_FORMAT_R8G8B8A8_USCALED`.
    pub const PIPE_FORMAT_R8G8B8A8_USCALED: Format = Format(92);
    /// `PIPE_FORMAT_R8_SSCALED`.
    pub const PIPE_FORMAT_R8_SSCALED: Format = Format(93);
    /// `PIPE_FORMAT_R8G8_SSCALED`.
    pub const PIPE_FORMAT_R8G8_SSCALED: Format = Format(94);
    /// `PIPE_FORMAT_R8G8B8_SSCALED`.
    pub const PIPE_FORMAT_R8G8B8_SSCALED: Format = Format(95);
    /// `PIPE_FORMAT_R8G8B8A8_SSCALED`.
    pub const PIPE_FORMAT_R8G8B8A8_SSCALED: Format = Format(96);
    /// `PIPE_FORMAT_B8G8R8_UINT`.
    pub const PIPE_FORMAT_B8G8R8_UINT: Format = Format(97);
    /// `PIPE_FORMAT_B8G8R8_SINT`.
    pub const PIPE_FORMAT_B8G8R8_SINT: Format = Format(98);
    /// `PIPE_FORMAT_B8G8R8_UNORM`.
    pub const PIPE_FORMAT_B8G8R8_UNORM: Format = Format(99);
    /// `PIPE_FORMAT_B8G8R8_SNORM`.
    pub const PIPE_FORMAT_B8G8R8_SNORM: Format = Format(100);
    /// `PIPE_FORMAT_B8G8R8_USCALED`.
    pub const PIPE_FORMAT_B8G8R8_USCALED: Format = Format(101);
    /// `PIPE_FORMAT_B8G8R8_SSCALED`.
    pub const PIPE_FORMAT_B8G8R8_SSCALED: Format = Format(102);
    /// `PIPE_FORMAT_B8G8R8A8_UINT`.
    pub const PIPE_FORMAT_B8G8R8A8_UINT: Format = Format(103);
    /// `PIPE_FORMAT_B8G8R8A8_SINT`.
    pub const PIPE_FORMAT_B8G8R8A8_SINT: Format = Format(104);
    /// `PIPE_FORMAT_B8G8R8A8_UNORM`.
    pub const PIPE_FORMAT_B8G8R8A8_UNORM: Format = Format(105);
    /// `PIPE_FORMAT_B8G8R8A8_SNORM`.
    pub const PIPE_FORMAT_B8G8R8A8_SNORM: Format = Format(106);
    /// `PIPE_FORMAT_B8G8R8A8_USCALED`.
    pub const PIPE_FORMAT_B8G8R8A8_USCALED: Format = Format(107);
    /// `PIPE_FORMAT_B8G8R8A8_SSCALED`.
    pub const PIPE_FORMAT_B8G8R8A8_SSCALED: Format = Format(108);
    /// `PIPE_FORMAT_A8B8G8R8_USCALED`.
    pub const PIPE_FORMAT_A8B8G8R8_USCALED: Format = Format(109);
    /// `PIPE_FORMAT_A8B8G8R8_SSCALED`.
    pub const PIPE_FORMAT_A8B8G8R8_SSCALED: Format = Format(110);
    /// `PIPE_FORMAT_A8R8G8B8_UNORM`.
    pub const PIPE_FORMAT_A8R8G8B8_UNORM: Format = Format(111);
    /// `PIPE_FORMAT_R10G10B10A2_UINT`.
    pub const PIPE_FORMAT_R10G10B10A2_UINT: Format = Format(112);
    /// `PIPE_FORMAT_R10G10B10A2_SINT`.
    pub const PIPE_FORMAT_R10G10B10A2_SINT: Format = Format(113);
    /// `PIPE_FORMAT_R10G10B10A2_UNORM`.
    pub const PIPE_FORMAT_R10G10B10A2_UNORM: Format = Format(114);
    /// `PIPE_FORMAT_R10G10B10A2_SNORM`.
    pub const PIPE_FORMAT_R10G10B10A2_SNORM: Format = Format(115);
    /// `PIPE_FORMAT_R10G10B10A2_USCALED`.
    pub const PIPE_FORMAT_R10G10B10A2_USCALED: Format = Format(116);
    /// `PIPE_FORMAT_R10G10B10A2_SSCALED`.
    pub const PIPE_FORMAT_R10G10B10A2_SSCALED: Format = Format(117);
    /// `PIPE_FORMAT_B10G10R10A2_UINT`.
    pub const PIPE_FORMAT_B10G10R10A2_UINT: Format = Format(118);
    /// `PIPE_FORMAT_B10G10R10A2_SINT`.
    pub const PIPE_FORMAT_B10G10R10A2_SINT: Format = Format(119);
    /// `PIPE_FORMAT_B10G10R10A2_UNORM`.
    pub const PIPE_FORMAT_B10G10R10A2_UNORM: Format = Format(120);
    /// `PIPE_FORMAT_B10G10R10A2_SNORM`.
    pub const PIPE_FORMAT_B10G10R10A2_SNORM: Format = Format(121);
    /// `PIPE_FORMAT_B10G10R10A2_USCALED`.
    pub const PIPE_FORMAT_B10G10R10A2_USCALED: Format = Format(122);
    /// `PIPE_FORMAT_B10G10R10A2_SSCALED`.
    pub const PIPE_FORMAT_B10G10R10A2_SSCALED: Format = Format(123);
    /// `PIPE_FORMAT_R11G11B10_FLOAT`.
    pub const PIPE_FORMAT_R11G11B10_FLOAT: Format = Format(124);
    /// `PIPE_FORMAT_VERTEX_END`.
    pub const PIPE_FORMAT_VERTEX_END: Format = Format(124);
    /// `PIPE_FORMAT_U8Y8V8Y8_422_UNORM`.
    pub const PIPE_FORMAT_U8Y8V8Y8_422_UNORM: Format = Format(125);
    /// `PIPE_FORMAT_YUV_START`.
    pub const PIPE_FORMAT_YUV_START: Format = Format(125);
    /// `PIPE_FORMAT_V8Y8U8Y8_422_UNORM`.
    pub const PIPE_FORMAT_V8Y8U8Y8_422_UNORM: Format = Format(126);
    /// `PIPE_FORMAT_Y8U8Y8V8_422_UNORM`.
    pub const PIPE_FORMAT_Y8U8Y8V8_422_UNORM: Format = Format(127);
    /// `PIPE_FORMAT_Y8V8Y8U8_422_UNORM`.
    pub const PIPE_FORMAT_Y8V8Y8U8_422_UNORM: Format = Format(128);
    /// `PIPE_FORMAT_A8Y8U8V8_444_UNORM`.
    pub const PIPE_FORMAT_A8Y8U8V8_444_UNORM: Format = Format(129);
    /// `PIPE_FORMAT_X8Y8U8V8_444_UNORM`.
    pub const PIPE_FORMAT_X8Y8U8V8_444_UNORM: Format = Format(130);
    /// `PIPE_FORMAT_Y8_V8_U8_420_UNORM`.
    pub const PIPE_FORMAT_Y8_V8_U8_420_UNORM: Format = Format(131);
    /// `PIPE_FORMAT_Y8_V8_U8_422_UNORM`.
    pub const PIPE_FORMAT_Y8_V8_U8_422_UNORM: Format = Format(132);
    /// `PIPE_FORMAT_Y8_U8_V8_420_UNORM`.
    pub const PIPE_FORMAT_Y8_U8_V8_420_UNORM: Format = Format(133);
    /// `PIPE_FORMAT_Y8_U8V8_420_UNORM`.
    pub const PIPE_FORMAT_Y8_U8V8_420_UNORM: Format = Format(134);
    /// `PIPE_FORMAT_Y8_V8U8_420_UNORM`.
    pub const PIPE_FORMAT_Y8_V8U8_420_UNORM: Format = Format(135);
    /// `PIPE_FORMAT_Y8_400_UNORM`.
    pub const PIPE_FORMAT_Y8_400_UNORM: Format = Format(136);
    /// `PIPE_FORMAT_Y10_U10V10_420_UNORM`.
    pub const PIPE_FORMAT_Y10_U10V10_420_UNORM: Format = Format(137);
    /// `PIPE_FORMAT_Y10_U10V10_422_UNORM`.
    pub const PIPE_FORMAT_Y10_U10V10_422_UNORM: Format = Format(138);
    /// `PIPE_FORMAT_Y16_U16V16_444_UNORM`.
    pub const PIPE_FORMAT_Y16_U16V16_444_UNORM: Format = Format(139);
    /// `PIPE_FORMAT_Y8_U8_V8_422_UNORM`.
    pub const PIPE_FORMAT_Y8_U8_V8_422_UNORM: Format = Format(140);
    /// `PIPE_FORMAT_Y8_U8V8_422_UNORM`.
    pub const PIPE_FORMAT_Y8_U8V8_422_UNORM: Format = Format(141);
    /// `PIPE_FORMAT_Y8_V8U8_422_UNORM`.
    pub const PIPE_FORMAT_Y8_V8U8_422_UNORM: Format = Format(142);
    /// `PIPE_FORMAT_Y8_U8V8_444_UNORM`.
    pub const PIPE_FORMAT_Y8_U8V8_444_UNORM: Format = Format(143);
    /// `PIPE_FORMAT_Y8_V8U8_444_UNORM`.
    pub const PIPE_FORMAT_Y8_V8U8_444_UNORM: Format = Format(144);
    /// `PIPE_FORMAT_Y8_U8_V8_444_UNORM`.
    pub const PIPE_FORMAT_Y8_U8_V8_444_UNORM: Format = Format(145);
    /// `PIPE_FORMAT_Y8_U8_V8_440_UNORM`.
    pub const PIPE_FORMAT_Y8_U8_V8_440_UNORM: Format = Format(146);
    /// `PIPE_FORMAT_Y10X6_U10X6_V10X6_420_UNORM`.
    pub const PIPE_FORMAT_Y10X6_U10X6_V10X6_420_UNORM: Format = Format(147);
    /// `PIPE_FORMAT_Y12X4_U12X4_V12X4_420_UNORM`.
    pub const PIPE_FORMAT_Y12X4_U12X4_V12X4_420_UNORM: Format = Format(148);
    /// `PIPE_FORMAT_Y16_U16_V16_420_UNORM`.
    pub const PIPE_FORMAT_Y16_U16_V16_420_UNORM: Format = Format(149);
    /// `PIPE_FORMAT_Y10X6_U10X6_V10X6_422_UNORM`.
    pub const PIPE_FORMAT_Y10X6_U10X6_V10X6_422_UNORM: Format = Format(150);
    /// `PIPE_FORMAT_Y12X4_U12X4_V12X4_422_UNORM`.
    pub const PIPE_FORMAT_Y12X4_U12X4_V12X4_422_UNORM: Format = Format(151);
    /// `PIPE_FORMAT_Y16_U16_V16_422_UNORM`.
    pub const PIPE_FORMAT_Y16_U16_V16_422_UNORM: Format = Format(152);
    /// `PIPE_FORMAT_Y10X6_U10X6_V10X6_444_UNORM`.
    pub const PIPE_FORMAT_Y10X6_U10X6_V10X6_444_UNORM: Format = Format(153);
    /// `PIPE_FORMAT_Y12X4_U12X4_V12X4_444_UNORM`.
    pub const PIPE_FORMAT_Y12X4_U12X4_V12X4_444_UNORM: Format = Format(154);
    /// `PIPE_FORMAT_Y16_U16_V16_444_UNORM`.
    pub const PIPE_FORMAT_Y16_U16_V16_444_UNORM: Format = Format(155);
    /// `PIPE_FORMAT_Y16_U16V16_422_UNORM`.
    pub const PIPE_FORMAT_Y16_U16V16_422_UNORM: Format = Format(156);
    /// `PIPE_FORMAT_X6Y10_X6U10X6V10_420_UNORM`.
    pub const PIPE_FORMAT_X6Y10_X6U10X6V10_420_UNORM: Format = Format(157);
    /// `PIPE_FORMAT_X4Y12_X4U12X4V12_420_UNORM`.
    pub const PIPE_FORMAT_X4Y12_X4U12X4V12_420_UNORM: Format = Format(158);
    /// `PIPE_FORMAT_Y16_U16V16_420_UNORM`.
    pub const PIPE_FORMAT_Y16_U16V16_420_UNORM: Format = Format(159);
    /// `PIPE_FORMAT_X6Y10_X6U10X6V10_422_UNORM`.
    pub const PIPE_FORMAT_X6Y10_X6U10X6V10_422_UNORM: Format = Format(160);
    /// `PIPE_FORMAT_Y10Y10Y10X2_U10V10U10X2V10U10V10X2_420_UNORM`.
    pub const PIPE_FORMAT_Y10Y10Y10X2_U10V10U10X2V10U10V10X2_420_UNORM: Format = Format(161);
    /// `PIPE_FORMAT_X6Y10X6U10X6Y10X6V10_422_UNORM`.
    pub const PIPE_FORMAT_X6Y10X6U10X6Y10X6V10_422_UNORM: Format = Format(162);
    /// `PIPE_FORMAT_X4Y12X4U12X4Y12X4V12_422_UNORM`.
    pub const PIPE_FORMAT_X4Y12X4U12X4Y12X4V12_422_UNORM: Format = Format(163);
    /// `PIPE_FORMAT_Y16U16Y16V16_422_UNORM`.
    pub const PIPE_FORMAT_Y16U16Y16V16_422_UNORM: Format = Format(164);
    /// `PIPE_FORMAT_U10Y10V10A2_444_UNORM`.
    pub const PIPE_FORMAT_U10Y10V10A2_444_UNORM: Format = Format(165);
    /// `PIPE_FORMAT_X4U12X4Y12X4V12X4A12_444_UNORM`.
    pub const PIPE_FORMAT_X4U12X4Y12X4V12X4A12_444_UNORM: Format = Format(166);
    /// `PIPE_FORMAT_U16Y16V16A16_444_UNORM`.
    pub const PIPE_FORMAT_U16Y16V16A16_444_UNORM: Format = Format(167);
    /// `PIPE_FORMAT_Y8U8V8_420_UNORM_PACKED`.
    pub const PIPE_FORMAT_Y8U8V8_420_UNORM_PACKED: Format = Format(168);
    /// `PIPE_FORMAT_Y10U10V10_420_UNORM_PACKED`.
    pub const PIPE_FORMAT_Y10U10V10_420_UNORM_PACKED: Format = Format(169);
    /// `PIPE_FORMAT_YUV_END`.
    pub const PIPE_FORMAT_YUV_END: Format = Format(169);
    /// `PIPE_FORMAT_DXT1_RGB`.
    pub const PIPE_FORMAT_DXT1_RGB: Format = Format(170);
    /// `PIPE_FORMAT_COMPR_START`.
    pub const PIPE_FORMAT_COMPR_START: Format = Format(170);
    /// `PIPE_FORMAT_DXT1_RGBA`.
    pub const PIPE_FORMAT_DXT1_RGBA: Format = Format(171);
    /// `PIPE_FORMAT_DXT3_RGBA`.
    pub const PIPE_FORMAT_DXT3_RGBA: Format = Format(172);
    /// `PIPE_FORMAT_DXT5_RGBA`.
    pub const PIPE_FORMAT_DXT5_RGBA: Format = Format(173);
    /// `PIPE_FORMAT_DXT1_SRGB`.
    pub const PIPE_FORMAT_DXT1_SRGB: Format = Format(174);
    /// `PIPE_FORMAT_DXT1_SRGBA`.
    pub const PIPE_FORMAT_DXT1_SRGBA: Format = Format(175);
    /// `PIPE_FORMAT_DXT3_SRGBA`.
    pub const PIPE_FORMAT_DXT3_SRGBA: Format = Format(176);
    /// `PIPE_FORMAT_DXT5_SRGBA`.
    pub const PIPE_FORMAT_DXT5_SRGBA: Format = Format(177);
    /// `PIPE_FORMAT_RGTC1_UNORM`.
    pub const PIPE_FORMAT_RGTC1_UNORM: Format = Format(178);
    /// `PIPE_FORMAT_RGTC1_SNORM`.
    pub const PIPE_FORMAT_RGTC1_SNORM: Format = Format(179);
    /// `PIPE_FORMAT_RGTC2_UNORM`.
    pub const PIPE_FORMAT_RGTC2_UNORM: Format = Format(180);
    /// `PIPE_FORMAT_RGTC2_SNORM`.
    pub const PIPE_FORMAT_RGTC2_SNORM: Format = Format(181);
    /// `PIPE_FORMAT_LATC1_UNORM`.
    pub const PIPE_FORMAT_LATC1_UNORM: Format = Format(182);
    /// `PIPE_FORMAT_LATC1_SNORM`.
    pub const PIPE_FORMAT_LATC1_SNORM: Format = Format(183);
    /// `PIPE_FORMAT_LATC2_UNORM`.
    pub const PIPE_FORMAT_LATC2_UNORM: Format = Format(184);
    /// `PIPE_FORMAT_LATC2_SNORM`.
    pub const PIPE_FORMAT_LATC2_SNORM: Format = Format(185);
    /// `PIPE_FORMAT_ETC1_RGB8`.
    pub const PIPE_FORMAT_ETC1_RGB8: Format = Format(186);
    /// `PIPE_FORMAT_ETC2_RGB8`.
    pub const PIPE_FORMAT_ETC2_RGB8: Format = Format(187);
    /// `PIPE_FORMAT_ETC2_RGB8A1`.
    pub const PIPE_FORMAT_ETC2_RGB8A1: Format = Format(188);
    /// `PIPE_FORMAT_ETC2_RGBA8`.
    pub const PIPE_FORMAT_ETC2_RGBA8: Format = Format(189);
    /// `PIPE_FORMAT_ETC2_R11_UNORM`.
    pub const PIPE_FORMAT_ETC2_R11_UNORM: Format = Format(190);
    /// `PIPE_FORMAT_ETC2_R11_SNORM`.
    pub const PIPE_FORMAT_ETC2_R11_SNORM: Format = Format(191);
    /// `PIPE_FORMAT_ETC2_RG11_UNORM`.
    pub const PIPE_FORMAT_ETC2_RG11_UNORM: Format = Format(192);
    /// `PIPE_FORMAT_ETC2_RG11_SNORM`.
    pub const PIPE_FORMAT_ETC2_RG11_SNORM: Format = Format(193);
    /// `PIPE_FORMAT_ETC2_SRGB8`.
    pub const PIPE_FORMAT_ETC2_SRGB8: Format = Format(194);
    /// `PIPE_FORMAT_ETC2_SRGB8A1`.
    pub const PIPE_FORMAT_ETC2_SRGB8A1: Format = Format(195);
    /// `PIPE_FORMAT_ETC2_SRGBA8`.
    pub const PIPE_FORMAT_ETC2_SRGBA8: Format = Format(196);
    /// `PIPE_FORMAT_BPTC_RGBA_UNORM`.
    pub const PIPE_FORMAT_BPTC_RGBA_UNORM: Format = Format(197);
    /// `PIPE_FORMAT_BPTC_RGB_FLOAT`.
    pub const PIPE_FORMAT_BPTC_RGB_FLOAT: Format = Format(198);
    /// `PIPE_FORMAT_BPTC_RGB_UFLOAT`.
    pub const PIPE_FORMAT_BPTC_RGB_UFLOAT: Format = Format(199);
    /// `PIPE_FORMAT_BPTC_SRGBA`.
    pub const PIPE_FORMAT_BPTC_SRGBA: Format = Format(200);
    /// `PIPE_FORMAT_ASTC_4x4`.
    pub const PIPE_FORMAT_ASTC_4X4: Format = Format(201);
    /// `PIPE_FORMAT_ASTC_5x4`.
    pub const PIPE_FORMAT_ASTC_5X4: Format = Format(202);
    /// `PIPE_FORMAT_ASTC_5x5`.
    pub const PIPE_FORMAT_ASTC_5X5: Format = Format(203);
    /// `PIPE_FORMAT_ASTC_6x5`.
    pub const PIPE_FORMAT_ASTC_6X5: Format = Format(204);
    /// `PIPE_FORMAT_ASTC_6x6`.
    pub const PIPE_FORMAT_ASTC_6X6: Format = Format(205);
    /// `PIPE_FORMAT_ASTC_8x5`.
    pub const PIPE_FORMAT_ASTC_8X5: Format = Format(206);
    /// `PIPE_FORMAT_ASTC_8x6`.
    pub const PIPE_FORMAT_ASTC_8X6: Format = Format(207);
    /// `PIPE_FORMAT_ASTC_8x8`.
    pub const PIPE_FORMAT_ASTC_8X8: Format = Format(208);
    /// `PIPE_FORMAT_ASTC_10x5`.
    pub const PIPE_FORMAT_ASTC_10X5: Format = Format(209);
    /// `PIPE_FORMAT_ASTC_10x6`.
    pub const PIPE_FORMAT_ASTC_10X6: Format = Format(210);
    /// `PIPE_FORMAT_ASTC_10x8`.
    pub const PIPE_FORMAT_ASTC_10X8: Format = Format(211);
    /// `PIPE_FORMAT_ASTC_10x10`.
    pub const PIPE_FORMAT_ASTC_10X10: Format = Format(212);
    /// `PIPE_FORMAT_ASTC_12x10`.
    pub const PIPE_FORMAT_ASTC_12X10: Format = Format(213);
    /// `PIPE_FORMAT_ASTC_12x12`.
    pub const PIPE_FORMAT_ASTC_12X12: Format = Format(214);
    /// `PIPE_FORMAT_ASTC_3x3x3`.
    pub const PIPE_FORMAT_ASTC_3X3X3: Format = Format(215);
    /// `PIPE_FORMAT_ASTC_4x3x3`.
    pub const PIPE_FORMAT_ASTC_4X3X3: Format = Format(216);
    /// `PIPE_FORMAT_ASTC_4x4x3`.
    pub const PIPE_FORMAT_ASTC_4X4X3: Format = Format(217);
    /// `PIPE_FORMAT_ASTC_4x4x4`.
    pub const PIPE_FORMAT_ASTC_4X4X4: Format = Format(218);
    /// `PIPE_FORMAT_ASTC_5x4x4`.
    pub const PIPE_FORMAT_ASTC_5X4X4: Format = Format(219);
    /// `PIPE_FORMAT_ASTC_5x5x4`.
    pub const PIPE_FORMAT_ASTC_5X5X4: Format = Format(220);
    /// `PIPE_FORMAT_ASTC_5x5x5`.
    pub const PIPE_FORMAT_ASTC_5X5X5: Format = Format(221);
    /// `PIPE_FORMAT_ASTC_6x5x5`.
    pub const PIPE_FORMAT_ASTC_6X5X5: Format = Format(222);
    /// `PIPE_FORMAT_ASTC_6x6x5`.
    pub const PIPE_FORMAT_ASTC_6X6X5: Format = Format(223);
    /// `PIPE_FORMAT_ASTC_6x6x6`.
    pub const PIPE_FORMAT_ASTC_6X6X6: Format = Format(224);
    /// `PIPE_FORMAT_ASTC_4x4_FLOAT`.
    pub const PIPE_FORMAT_ASTC_4X4_FLOAT: Format = Format(225);
    /// `PIPE_FORMAT_ASTC_5x4_FLOAT`.
    pub const PIPE_FORMAT_ASTC_5X4_FLOAT: Format = Format(226);
    /// `PIPE_FORMAT_ASTC_5x5_FLOAT`.
    pub const PIPE_FORMAT_ASTC_5X5_FLOAT: Format = Format(227);
    /// `PIPE_FORMAT_ASTC_6x5_FLOAT`.
    pub const PIPE_FORMAT_ASTC_6X5_FLOAT: Format = Format(228);
    /// `PIPE_FORMAT_ASTC_6x6_FLOAT`.
    pub const PIPE_FORMAT_ASTC_6X6_FLOAT: Format = Format(229);
    /// `PIPE_FORMAT_ASTC_8x5_FLOAT`.
    pub const PIPE_FORMAT_ASTC_8X5_FLOAT: Format = Format(230);
    /// `PIPE_FORMAT_ASTC_8x6_FLOAT`.
    pub const PIPE_FORMAT_ASTC_8X6_FLOAT: Format = Format(231);
    /// `PIPE_FORMAT_ASTC_8x8_FLOAT`.
    pub const PIPE_FORMAT_ASTC_8X8_FLOAT: Format = Format(232);
    /// `PIPE_FORMAT_ASTC_10x5_FLOAT`.
    pub const PIPE_FORMAT_ASTC_10X5_FLOAT: Format = Format(233);
    /// `PIPE_FORMAT_ASTC_10x6_FLOAT`.
    pub const PIPE_FORMAT_ASTC_10X6_FLOAT: Format = Format(234);
    /// `PIPE_FORMAT_ASTC_10x8_FLOAT`.
    pub const PIPE_FORMAT_ASTC_10X8_FLOAT: Format = Format(235);
    /// `PIPE_FORMAT_ASTC_10x10_FLOAT`.
    pub const PIPE_FORMAT_ASTC_10X10_FLOAT: Format = Format(236);
    /// `PIPE_FORMAT_ASTC_12x10_FLOAT`.
    pub const PIPE_FORMAT_ASTC_12X10_FLOAT: Format = Format(237);
    /// `PIPE_FORMAT_ASTC_12x12_FLOAT`.
    pub const PIPE_FORMAT_ASTC_12X12_FLOAT: Format = Format(238);
    /// `PIPE_FORMAT_ASTC_4x4_SRGB`.
    pub const PIPE_FORMAT_ASTC_4X4_SRGB: Format = Format(239);
    /// `PIPE_FORMAT_ASTC_5x4_SRGB`.
    pub const PIPE_FORMAT_ASTC_5X4_SRGB: Format = Format(240);
    /// `PIPE_FORMAT_ASTC_5x5_SRGB`.
    pub const PIPE_FORMAT_ASTC_5X5_SRGB: Format = Format(241);
    /// `PIPE_FORMAT_ASTC_6x5_SRGB`.
    pub const PIPE_FORMAT_ASTC_6X5_SRGB: Format = Format(242);
    /// `PIPE_FORMAT_ASTC_6x6_SRGB`.
    pub const PIPE_FORMAT_ASTC_6X6_SRGB: Format = Format(243);
    /// `PIPE_FORMAT_ASTC_8x5_SRGB`.
    pub const PIPE_FORMAT_ASTC_8X5_SRGB: Format = Format(244);
    /// `PIPE_FORMAT_ASTC_8x6_SRGB`.
    pub const PIPE_FORMAT_ASTC_8X6_SRGB: Format = Format(245);
    /// `PIPE_FORMAT_ASTC_8x8_SRGB`.
    pub const PIPE_FORMAT_ASTC_8X8_SRGB: Format = Format(246);
    /// `PIPE_FORMAT_ASTC_10x5_SRGB`.
    pub const PIPE_FORMAT_ASTC_10X5_SRGB: Format = Format(247);
    /// `PIPE_FORMAT_ASTC_10x6_SRGB`.
    pub const PIPE_FORMAT_ASTC_10X6_SRGB: Format = Format(248);
    /// `PIPE_FORMAT_ASTC_10x8_SRGB`.
    pub const PIPE_FORMAT_ASTC_10X8_SRGB: Format = Format(249);
    /// `PIPE_FORMAT_ASTC_10x10_SRGB`.
    pub const PIPE_FORMAT_ASTC_10X10_SRGB: Format = Format(250);
    /// `PIPE_FORMAT_ASTC_12x10_SRGB`.
    pub const PIPE_FORMAT_ASTC_12X10_SRGB: Format = Format(251);
    /// `PIPE_FORMAT_ASTC_12x12_SRGB`.
    pub const PIPE_FORMAT_ASTC_12X12_SRGB: Format = Format(252);
    /// `PIPE_FORMAT_ASTC_3x3x3_SRGB`.
    pub const PIPE_FORMAT_ASTC_3X3X3_SRGB: Format = Format(253);
    /// `PIPE_FORMAT_ASTC_4x3x3_SRGB`.
    pub const PIPE_FORMAT_ASTC_4X3X3_SRGB: Format = Format(254);
    /// `PIPE_FORMAT_ASTC_4x4x3_SRGB`.
    pub const PIPE_FORMAT_ASTC_4X4X3_SRGB: Format = Format(255);
    /// `PIPE_FORMAT_ASTC_4x4x4_SRGB`.
    pub const PIPE_FORMAT_ASTC_4X4X4_SRGB: Format = Format(256);
    /// `PIPE_FORMAT_ASTC_5x4x4_SRGB`.
    pub const PIPE_FORMAT_ASTC_5X4X4_SRGB: Format = Format(257);
    /// `PIPE_FORMAT_ASTC_5x5x4_SRGB`.
    pub const PIPE_FORMAT_ASTC_5X5X4_SRGB: Format = Format(258);
    /// `PIPE_FORMAT_ASTC_5x5x5_SRGB`.
    pub const PIPE_FORMAT_ASTC_5X5X5_SRGB: Format = Format(259);
    /// `PIPE_FORMAT_ASTC_6x5x5_SRGB`.
    pub const PIPE_FORMAT_ASTC_6X5X5_SRGB: Format = Format(260);
    /// `PIPE_FORMAT_ASTC_6x6x5_SRGB`.
    pub const PIPE_FORMAT_ASTC_6X6X5_SRGB: Format = Format(261);
    /// `PIPE_FORMAT_ASTC_6x6x6_SRGB`.
    pub const PIPE_FORMAT_ASTC_6X6X6_SRGB: Format = Format(262);
    /// `PIPE_FORMAT_ATC_RGB`.
    pub const PIPE_FORMAT_ATC_RGB: Format = Format(263);
    /// `PIPE_FORMAT_ATC_RGBA_EXPLICIT`.
    pub const PIPE_FORMAT_ATC_RGBA_EXPLICIT: Format = Format(264);
    /// `PIPE_FORMAT_ATC_RGBA_INTERPOLATED`.
    pub const PIPE_FORMAT_ATC_RGBA_INTERPOLATED: Format = Format(265);
    /// `PIPE_FORMAT_FXT1_RGB`.
    pub const PIPE_FORMAT_FXT1_RGB: Format = Format(266);
    /// `PIPE_FORMAT_FXT1_RGBA`.
    pub const PIPE_FORMAT_FXT1_RGBA: Format = Format(267);
    /// `PIPE_FORMAT_COMPR_END`.
    pub const PIPE_FORMAT_COMPR_END: Format = Format(267);
    /// `PIPE_FORMAT_S8_UINT`.
    pub const PIPE_FORMAT_S8_UINT: Format = Format(268);
    /// `PIPE_FORMAT_ZS_START`.
    pub const PIPE_FORMAT_ZS_START: Format = Format(268);
    /// `PIPE_FORMAT_Z16_UNORM`.
    pub const PIPE_FORMAT_Z16_UNORM: Format = Format(269);
    /// `PIPE_FORMAT_Z16_UNORM_S8_UINT`.
    pub const PIPE_FORMAT_Z16_UNORM_S8_UINT: Format = Format(270);
    /// `PIPE_FORMAT_Z32_UNORM`.
    pub const PIPE_FORMAT_Z32_UNORM: Format = Format(271);
    /// `PIPE_FORMAT_Z32_FLOAT`.
    pub const PIPE_FORMAT_Z32_FLOAT: Format = Format(272);
    /// `PIPE_FORMAT_Z24_UNORM_S8_UINT`.
    pub const PIPE_FORMAT_Z24_UNORM_S8_UINT: Format = Format(273);
    /// `PIPE_FORMAT_S8_UINT_Z24_UNORM`.
    pub const PIPE_FORMAT_S8_UINT_Z24_UNORM: Format = Format(274);
    /// `PIPE_FORMAT_X24S8_UINT`.
    pub const PIPE_FORMAT_X24S8_UINT: Format = Format(275);
    /// `PIPE_FORMAT_S8X24_UINT`.
    pub const PIPE_FORMAT_S8X24_UINT: Format = Format(276);
    /// `PIPE_FORMAT_Z24X8_UNORM`.
    pub const PIPE_FORMAT_Z24X8_UNORM: Format = Format(277);
    /// `PIPE_FORMAT_X8Z24_UNORM`.
    pub const PIPE_FORMAT_X8Z24_UNORM: Format = Format(278);
    /// `PIPE_FORMAT_Z24_UNORM_PACKED`.
    pub const PIPE_FORMAT_Z24_UNORM_PACKED: Format = Format(279);
    /// `PIPE_FORMAT_Z32_FLOAT_S8X24_UINT`.
    pub const PIPE_FORMAT_Z32_FLOAT_S8X24_UINT: Format = Format(280);
    /// `PIPE_FORMAT_X32_S8X24_UINT`.
    pub const PIPE_FORMAT_X32_S8X24_UINT: Format = Format(281);
    /// `PIPE_FORMAT_ZS_END`.
    pub const PIPE_FORMAT_ZS_END: Format = Format(281);
    /// `PIPE_FORMAT_B8G8R8X8_UNORM`.
    pub const PIPE_FORMAT_B8G8R8X8_UNORM: Format = Format(282);
    /// `PIPE_FORMAT_B8G8R8X8_SNORM`.
    pub const PIPE_FORMAT_B8G8R8X8_SNORM: Format = Format(283);
    /// `PIPE_FORMAT_B8G8R8X8_UINT`.
    pub const PIPE_FORMAT_B8G8R8X8_UINT: Format = Format(284);
    /// `PIPE_FORMAT_B8G8R8X8_SINT`.
    pub const PIPE_FORMAT_B8G8R8X8_SINT: Format = Format(285);
    /// `PIPE_FORMAT_A8R8G8B8_SNORM`.
    pub const PIPE_FORMAT_A8R8G8B8_SNORM: Format = Format(286);
    /// `PIPE_FORMAT_A8R8G8B8_SINT`.
    pub const PIPE_FORMAT_A8R8G8B8_SINT: Format = Format(287);
    /// `PIPE_FORMAT_X8R8G8B8_UNORM`.
    pub const PIPE_FORMAT_X8R8G8B8_UNORM: Format = Format(288);
    /// `PIPE_FORMAT_X8R8G8B8_SNORM`.
    pub const PIPE_FORMAT_X8R8G8B8_SNORM: Format = Format(289);
    /// `PIPE_FORMAT_X8R8G8B8_SINT`.
    pub const PIPE_FORMAT_X8R8G8B8_SINT: Format = Format(290);
    /// `PIPE_FORMAT_A8B8G8R8_UNORM`.
    pub const PIPE_FORMAT_A8B8G8R8_UNORM: Format = Format(291);
    /// `PIPE_FORMAT_X8B8G8R8_UNORM`.
    pub const PIPE_FORMAT_X8B8G8R8_UNORM: Format = Format(292);
    /// `PIPE_FORMAT_R8G8B8X8_UNORM`.
    pub const PIPE_FORMAT_R8G8B8X8_UNORM: Format = Format(293);
    /// `PIPE_FORMAT_R5G5B5A1_UNORM`.
    pub const PIPE_FORMAT_R5G5B5A1_UNORM: Format = Format(294);
    /// `PIPE_FORMAT_R5G5B5X1_UNORM`.
    pub const PIPE_FORMAT_R5G5B5X1_UNORM: Format = Format(295);
    /// `PIPE_FORMAT_B5G5R5X1_UNORM`.
    pub const PIPE_FORMAT_B5G5R5X1_UNORM: Format = Format(296);
    /// `PIPE_FORMAT_B5G5R5A1_UNORM`.
    pub const PIPE_FORMAT_B5G5R5A1_UNORM: Format = Format(297);
    /// `PIPE_FORMAT_X1B5G5R5_UNORM`.
    pub const PIPE_FORMAT_X1B5G5R5_UNORM: Format = Format(298);
    /// `PIPE_FORMAT_A1R5G5B5_UNORM`.
    pub const PIPE_FORMAT_A1R5G5B5_UNORM: Format = Format(299);
    /// `PIPE_FORMAT_X1R5G5B5_UNORM`.
    pub const PIPE_FORMAT_X1R5G5B5_UNORM: Format = Format(300);
    /// `PIPE_FORMAT_A1B5G5R5_UNORM`.
    pub const PIPE_FORMAT_A1B5G5R5_UNORM: Format = Format(301);
    /// `PIPE_FORMAT_R4G4B4A4_UNORM`.
    pub const PIPE_FORMAT_R4G4B4A4_UNORM: Format = Format(302);
    /// `PIPE_FORMAT_R4G4B4X4_UNORM`.
    pub const PIPE_FORMAT_R4G4B4X4_UNORM: Format = Format(303);
    /// `PIPE_FORMAT_B4G4R4A4_UNORM`.
    pub const PIPE_FORMAT_B4G4R4A4_UNORM: Format = Format(304);
    /// `PIPE_FORMAT_B4G4R4X4_UNORM`.
    pub const PIPE_FORMAT_B4G4R4X4_UNORM: Format = Format(305);
    /// `PIPE_FORMAT_A4R4G4B4_UNORM`.
    pub const PIPE_FORMAT_A4R4G4B4_UNORM: Format = Format(306);
    /// `PIPE_FORMAT_A4B4G4R4_UNORM`.
    pub const PIPE_FORMAT_A4B4G4R4_UNORM: Format = Format(307);
    /// `PIPE_FORMAT_R5G6B5_UNORM`.
    pub const PIPE_FORMAT_R5G6B5_UNORM: Format = Format(308);
    /// `PIPE_FORMAT_B5G6R5_UNORM`.
    pub const PIPE_FORMAT_B5G6R5_UNORM: Format = Format(309);
    /// `PIPE_FORMAT_R10G10B10X2_UNORM`.
    pub const PIPE_FORMAT_R10G10B10X2_UNORM: Format = Format(310);
    /// `PIPE_FORMAT_A2R10G10B10_UNORM`.
    pub const PIPE_FORMAT_A2R10G10B10_UNORM: Format = Format(311);
    /// `PIPE_FORMAT_A2B10G10R10_UNORM`.
    pub const PIPE_FORMAT_A2B10G10R10_UNORM: Format = Format(312);
    /// `PIPE_FORMAT_R3G3B2_UNORM`.
    pub const PIPE_FORMAT_R3G3B2_UNORM: Format = Format(313);
    /// `PIPE_FORMAT_B2G3R3_UNORM`.
    pub const PIPE_FORMAT_B2G3R3_UNORM: Format = Format(314);
    /// `PIPE_FORMAT_L8_UNORM`.
    pub const PIPE_FORMAT_L8_UNORM: Format = Format(315);
    /// `PIPE_FORMAT_A8_UNORM`.
    pub const PIPE_FORMAT_A8_UNORM: Format = Format(316);
    /// `PIPE_FORMAT_I8_UNORM`.
    pub const PIPE_FORMAT_I8_UNORM: Format = Format(317);
    /// `PIPE_FORMAT_L4A4_UNORM`.
    pub const PIPE_FORMAT_L4A4_UNORM: Format = Format(318);
    /// `PIPE_FORMAT_L8A8_UNORM`.
    pub const PIPE_FORMAT_L8A8_UNORM: Format = Format(319);
    /// `PIPE_FORMAT_L16_UNORM`.
    pub const PIPE_FORMAT_L16_UNORM: Format = Format(320);
    /// `PIPE_FORMAT_A16_UNORM`.
    pub const PIPE_FORMAT_A16_UNORM: Format = Format(321);
    /// `PIPE_FORMAT_I16_UNORM`.
    pub const PIPE_FORMAT_I16_UNORM: Format = Format(322);
    /// `PIPE_FORMAT_L16A16_UNORM`.
    pub const PIPE_FORMAT_L16A16_UNORM: Format = Format(323);
    /// `PIPE_FORMAT_A8_SNORM`.
    pub const PIPE_FORMAT_A8_SNORM: Format = Format(324);
    /// `PIPE_FORMAT_L8_SNORM`.
    pub const PIPE_FORMAT_L8_SNORM: Format = Format(325);
    /// `PIPE_FORMAT_L8A8_SNORM`.
    pub const PIPE_FORMAT_L8A8_SNORM: Format = Format(326);
    /// `PIPE_FORMAT_I8_SNORM`.
    pub const PIPE_FORMAT_I8_SNORM: Format = Format(327);
    /// `PIPE_FORMAT_A16_SNORM`.
    pub const PIPE_FORMAT_A16_SNORM: Format = Format(328);
    /// `PIPE_FORMAT_L16_SNORM`.
    pub const PIPE_FORMAT_L16_SNORM: Format = Format(329);
    /// `PIPE_FORMAT_L16A16_SNORM`.
    pub const PIPE_FORMAT_L16A16_SNORM: Format = Format(330);
    /// `PIPE_FORMAT_I16_SNORM`.
    pub const PIPE_FORMAT_I16_SNORM: Format = Format(331);
    /// `PIPE_FORMAT_A16_FLOAT`.
    pub const PIPE_FORMAT_A16_FLOAT: Format = Format(332);
    /// `PIPE_FORMAT_L16_FLOAT`.
    pub const PIPE_FORMAT_L16_FLOAT: Format = Format(333);
    /// `PIPE_FORMAT_L16A16_FLOAT`.
    pub const PIPE_FORMAT_L16A16_FLOAT: Format = Format(334);
    /// `PIPE_FORMAT_I16_FLOAT`.
    pub const PIPE_FORMAT_I16_FLOAT: Format = Format(335);
    /// `PIPE_FORMAT_A32_FLOAT`.
    pub const PIPE_FORMAT_A32_FLOAT: Format = Format(336);
    /// `PIPE_FORMAT_L32_FLOAT`.
    pub const PIPE_FORMAT_L32_FLOAT: Format = Format(337);
    /// `PIPE_FORMAT_L32A32_FLOAT`.
    pub const PIPE_FORMAT_L32A32_FLOAT: Format = Format(338);
    /// `PIPE_FORMAT_I32_FLOAT`.
    pub const PIPE_FORMAT_I32_FLOAT: Format = Format(339);
    /// `PIPE_FORMAT_L8_SRGB`.
    pub const PIPE_FORMAT_L8_SRGB: Format = Format(340);
    /// `PIPE_FORMAT_R8_SRGB`.
    pub const PIPE_FORMAT_R8_SRGB: Format = Format(341);
    /// `PIPE_FORMAT_L8A8_SRGB`.
    pub const PIPE_FORMAT_L8A8_SRGB: Format = Format(342);
    /// `PIPE_FORMAT_R8G8_SRGB`.
    pub const PIPE_FORMAT_R8G8_SRGB: Format = Format(343);
    /// `PIPE_FORMAT_R8G8B8_SRGB`.
    pub const PIPE_FORMAT_R8G8B8_SRGB: Format = Format(344);
    /// `PIPE_FORMAT_B8G8R8_SRGB`.
    pub const PIPE_FORMAT_B8G8R8_SRGB: Format = Format(345);
    /// `PIPE_FORMAT_R8G8B8A8_SRGB`.
    pub const PIPE_FORMAT_R8G8B8A8_SRGB: Format = Format(346);
    /// `PIPE_FORMAT_A8B8G8R8_SRGB`.
    pub const PIPE_FORMAT_A8B8G8R8_SRGB: Format = Format(347);
    /// `PIPE_FORMAT_X8B8G8R8_SRGB`.
    pub const PIPE_FORMAT_X8B8G8R8_SRGB: Format = Format(348);
    /// `PIPE_FORMAT_B8G8R8A8_SRGB`.
    pub const PIPE_FORMAT_B8G8R8A8_SRGB: Format = Format(349);
    /// `PIPE_FORMAT_B8G8R8X8_SRGB`.
    pub const PIPE_FORMAT_B8G8R8X8_SRGB: Format = Format(350);
    /// `PIPE_FORMAT_A8R8G8B8_SRGB`.
    pub const PIPE_FORMAT_A8R8G8B8_SRGB: Format = Format(351);
    /// `PIPE_FORMAT_X8R8G8B8_SRGB`.
    pub const PIPE_FORMAT_X8R8G8B8_SRGB: Format = Format(352);
    /// `PIPE_FORMAT_R8SG8SB8UX8U_NORM`.
    pub const PIPE_FORMAT_R8SG8SB8UX8U_NORM: Format = Format(353);
    /// `PIPE_FORMAT_R10SG10SB10SA2U_NORM`.
    pub const PIPE_FORMAT_R10SG10SB10SA2U_NORM: Format = Format(354);
    /// `PIPE_FORMAT_R5SG5SB6U_NORM`.
    pub const PIPE_FORMAT_R5SG5SB6U_NORM: Format = Format(355);
    /// `PIPE_FORMAT_Z24_UNORM_S8_UINT_AS_R8G8B8A8`.
    pub const PIPE_FORMAT_Z24_UNORM_S8_UINT_AS_R8G8B8A8: Format = Format(356);
    /// `PIPE_FORMAT_R8G8_B8G8_422_UNORM`.
    pub const PIPE_FORMAT_R8G8_B8G8_422_UNORM: Format = Format(357);
    /// `PIPE_FORMAT_G8R8_G8B8_422_UNORM`.
    pub const PIPE_FORMAT_G8R8_G8B8_422_UNORM: Format = Format(358);
    /// `PIPE_FORMAT_G8R8_B8R8_422_UNORM`.
    pub const PIPE_FORMAT_G8R8_B8R8_422_UNORM: Format = Format(359);
    /// `PIPE_FORMAT_R8G8_R8B8_422_UNORM`.
    pub const PIPE_FORMAT_R8G8_R8B8_422_UNORM: Format = Format(360);
    /// `PIPE_FORMAT_B8R8_G8R8_422_UNORM`.
    pub const PIPE_FORMAT_B8R8_G8R8_422_UNORM: Format = Format(361);
    /// `PIPE_FORMAT_R8B8_R8G8_422_UNORM`.
    pub const PIPE_FORMAT_R8B8_R8G8_422_UNORM: Format = Format(362);
    /// `PIPE_FORMAT_G8B8_G8R8_422_UNORM`.
    pub const PIPE_FORMAT_G8B8_G8R8_422_UNORM: Format = Format(363);
    /// `PIPE_FORMAT_B8G8_R8G8_422_UNORM`.
    pub const PIPE_FORMAT_B8G8_R8G8_422_UNORM: Format = Format(364);
    /// `PIPE_FORMAT_R16G16_R16B16_422_UNORM`.
    pub const PIPE_FORMAT_R16G16_R16B16_422_UNORM: Format = Format(365);
    /// `PIPE_FORMAT_X6R10X6G10_X6R10X6B10_422_UNORM`.
    pub const PIPE_FORMAT_X6R10X6G10_X6R10X6B10_422_UNORM: Format = Format(366);
    /// `PIPE_FORMAT_R9G9B9E5_FLOAT`.
    pub const PIPE_FORMAT_R9G9B9E5_FLOAT: Format = Format(367);
    /// `PIPE_FORMAT_R1_UNORM`.
    pub const PIPE_FORMAT_R1_UNORM: Format = Format(368);
    /// `PIPE_FORMAT_R8G8Bx_SNORM`.
    pub const PIPE_FORMAT_R8G8BX_SNORM: Format = Format(369);
    /// `PIPE_FORMAT_R10G10B10X2_USCALED`.
    pub const PIPE_FORMAT_R10G10B10X2_USCALED: Format = Format(370);
    /// `PIPE_FORMAT_R10G10B10X2_SNORM`.
    pub const PIPE_FORMAT_R10G10B10X2_SNORM: Format = Format(371);
    /// `PIPE_FORMAT_R10G10B10X2_SINT`.
    pub const PIPE_FORMAT_R10G10B10X2_SINT: Format = Format(372);
    /// `PIPE_FORMAT_R8_G8B8_420_UNORM`.
    pub const PIPE_FORMAT_R8_G8B8_420_UNORM: Format = Format(373);
    /// `PIPE_FORMAT_R8_B8G8_420_UNORM`.
    pub const PIPE_FORMAT_R8_B8G8_420_UNORM: Format = Format(374);
    /// `PIPE_FORMAT_G8_B8R8_420_UNORM`.
    pub const PIPE_FORMAT_G8_B8R8_420_UNORM: Format = Format(375);
    /// `PIPE_FORMAT_R8_G8B8_422_UNORM`.
    pub const PIPE_FORMAT_R8_G8B8_422_UNORM: Format = Format(376);
    /// `PIPE_FORMAT_R8_B8G8_422_UNORM`.
    pub const PIPE_FORMAT_R8_B8G8_422_UNORM: Format = Format(377);
    /// `PIPE_FORMAT_G8_B8R8_422_UNORM`.
    pub const PIPE_FORMAT_G8_B8R8_422_UNORM: Format = Format(378);
    /// `PIPE_FORMAT_R10_G10B10_420_UNORM`.
    pub const PIPE_FORMAT_R10_G10B10_420_UNORM: Format = Format(379);
    /// `PIPE_FORMAT_R10_G10B10_422_UNORM`.
    pub const PIPE_FORMAT_R10_G10B10_422_UNORM: Format = Format(380);
    /// `PIPE_FORMAT_R8_G8_B8_420_UNORM`.
    pub const PIPE_FORMAT_R8_G8_B8_420_UNORM: Format = Format(381);
    /// `PIPE_FORMAT_R8_B8_G8_420_UNORM`.
    pub const PIPE_FORMAT_R8_B8_G8_420_UNORM: Format = Format(382);
    /// `PIPE_FORMAT_G8_B8_R8_420_UNORM`.
    pub const PIPE_FORMAT_G8_B8_R8_420_UNORM: Format = Format(383);
    /// `PIPE_FORMAT_R8_G8_B8_444_UNORM`.
    pub const PIPE_FORMAT_R8_G8_B8_444_UNORM: Format = Format(384);
    /// `PIPE_FORMAT_X6G10_X6B10X6R10_420_UNORM`.
    pub const PIPE_FORMAT_X6G10_X6B10X6R10_420_UNORM: Format = Format(385);
    /// `PIPE_FORMAT_X4G12_X4B12X4R12_420_UNORM`.
    pub const PIPE_FORMAT_X4G12_X4B12X4R12_420_UNORM: Format = Format(386);
    /// `PIPE_FORMAT_X6G10_X6B10X6R10_422_UNORM`.
    pub const PIPE_FORMAT_X6G10_X6B10X6R10_422_UNORM: Format = Format(387);
    /// `PIPE_FORMAT_G8_B8R8_444_UNORM`.
    pub const PIPE_FORMAT_G8_B8R8_444_UNORM: Format = Format(388);
    /// `PIPE_FORMAT_X6G10_X6B10X6R10_444_UNORM`.
    pub const PIPE_FORMAT_X6G10_X6B10X6R10_444_UNORM: Format = Format(389);
    /// `PIPE_FORMAT_X4G12_X4B12X4R12_444_UNORM`.
    pub const PIPE_FORMAT_X4G12_X4B12X4R12_444_UNORM: Format = Format(390);
    /// `PIPE_FORMAT_R8G8B8_420_UNORM_PACKED`.
    pub const PIPE_FORMAT_R8G8B8_420_UNORM_PACKED: Format = Format(391);
    /// `PIPE_FORMAT_R10G10B10_420_UNORM_PACKED`.
    pub const PIPE_FORMAT_R10G10B10_420_UNORM_PACKED: Format = Format(392);
    /// `PIPE_FORMAT_X6R10X6G10X6B10X6A10_UNORM`.
    pub const PIPE_FORMAT_X6R10X6G10X6B10X6A10_UNORM: Format = Format(393);
    /// `PIPE_FORMAT_X6R10_UNORM`.
    pub const PIPE_FORMAT_X6R10_UNORM: Format = Format(394);
    /// `PIPE_FORMAT_X6R10X6G10_UNORM`.
    pub const PIPE_FORMAT_X6R10X6G10_UNORM: Format = Format(395);
    /// `PIPE_FORMAT_X4R12_UNORM`.
    pub const PIPE_FORMAT_X4R12_UNORM: Format = Format(396);
    /// `PIPE_FORMAT_X4R12X4G12_UNORM`.
    pub const PIPE_FORMAT_X4R12X4G12_UNORM: Format = Format(397);
    /// `PIPE_FORMAT_A4R4_UNORM`.
    pub const PIPE_FORMAT_A4R4_UNORM: Format = Format(398);
    /// `PIPE_FORMAT_R4A4_UNORM`.
    pub const PIPE_FORMAT_R4A4_UNORM: Format = Format(399);
    /// `PIPE_FORMAT_R8A8_UNORM`.
    pub const PIPE_FORMAT_R8A8_UNORM: Format = Format(400);
    /// `PIPE_FORMAT_A8R8_UNORM`.
    pub const PIPE_FORMAT_A8R8_UNORM: Format = Format(401);
    /// `PIPE_FORMAT_A8_UINT`.
    pub const PIPE_FORMAT_A8_UINT: Format = Format(402);
    /// `PIPE_FORMAT_I8_UINT`.
    pub const PIPE_FORMAT_I8_UINT: Format = Format(403);
    /// `PIPE_FORMAT_L8_UINT`.
    pub const PIPE_FORMAT_L8_UINT: Format = Format(404);
    /// `PIPE_FORMAT_L8A8_UINT`.
    pub const PIPE_FORMAT_L8A8_UINT: Format = Format(405);
    /// `PIPE_FORMAT_A8_SINT`.
    pub const PIPE_FORMAT_A8_SINT: Format = Format(406);
    /// `PIPE_FORMAT_I8_SINT`.
    pub const PIPE_FORMAT_I8_SINT: Format = Format(407);
    /// `PIPE_FORMAT_L8_SINT`.
    pub const PIPE_FORMAT_L8_SINT: Format = Format(408);
    /// `PIPE_FORMAT_L8A8_SINT`.
    pub const PIPE_FORMAT_L8A8_SINT: Format = Format(409);
    /// `PIPE_FORMAT_A16_UINT`.
    pub const PIPE_FORMAT_A16_UINT: Format = Format(410);
    /// `PIPE_FORMAT_I16_UINT`.
    pub const PIPE_FORMAT_I16_UINT: Format = Format(411);
    /// `PIPE_FORMAT_L16_UINT`.
    pub const PIPE_FORMAT_L16_UINT: Format = Format(412);
    /// `PIPE_FORMAT_L16A16_UINT`.
    pub const PIPE_FORMAT_L16A16_UINT: Format = Format(413);
    /// `PIPE_FORMAT_A16_SINT`.
    pub const PIPE_FORMAT_A16_SINT: Format = Format(414);
    /// `PIPE_FORMAT_I16_SINT`.
    pub const PIPE_FORMAT_I16_SINT: Format = Format(415);
    /// `PIPE_FORMAT_L16_SINT`.
    pub const PIPE_FORMAT_L16_SINT: Format = Format(416);
    /// `PIPE_FORMAT_L16A16_SINT`.
    pub const PIPE_FORMAT_L16A16_SINT: Format = Format(417);
    /// `PIPE_FORMAT_A32_UINT`.
    pub const PIPE_FORMAT_A32_UINT: Format = Format(418);
    /// `PIPE_FORMAT_I32_UINT`.
    pub const PIPE_FORMAT_I32_UINT: Format = Format(419);
    /// `PIPE_FORMAT_L32_UINT`.
    pub const PIPE_FORMAT_L32_UINT: Format = Format(420);
    /// `PIPE_FORMAT_L32A32_UINT`.
    pub const PIPE_FORMAT_L32A32_UINT: Format = Format(421);
    /// `PIPE_FORMAT_A32_SINT`.
    pub const PIPE_FORMAT_A32_SINT: Format = Format(422);
    /// `PIPE_FORMAT_I32_SINT`.
    pub const PIPE_FORMAT_I32_SINT: Format = Format(423);
    /// `PIPE_FORMAT_L32_SINT`.
    pub const PIPE_FORMAT_L32_SINT: Format = Format(424);
    /// `PIPE_FORMAT_L32A32_SINT`.
    pub const PIPE_FORMAT_L32A32_SINT: Format = Format(425);
    /// `PIPE_FORMAT_A8R8G8B8_UINT`.
    pub const PIPE_FORMAT_A8R8G8B8_UINT: Format = Format(426);
    /// `PIPE_FORMAT_A8B8G8R8_UINT`.
    pub const PIPE_FORMAT_A8B8G8R8_UINT: Format = Format(427);
    /// `PIPE_FORMAT_A2R10G10B10_UINT`.
    pub const PIPE_FORMAT_A2R10G10B10_UINT: Format = Format(428);
    /// `PIPE_FORMAT_A2B10G10R10_UINT`.
    pub const PIPE_FORMAT_A2B10G10R10_UINT: Format = Format(429);
    /// `PIPE_FORMAT_R5G6B5_UINT`.
    pub const PIPE_FORMAT_R5G6B5_UINT: Format = Format(430);
    /// `PIPE_FORMAT_B5G6R5_UINT`.
    pub const PIPE_FORMAT_B5G6R5_UINT: Format = Format(431);
    /// `PIPE_FORMAT_R3G3B2_UINT`.
    pub const PIPE_FORMAT_R3G3B2_UINT: Format = Format(432);
    /// `PIPE_FORMAT_B2G3R3_UINT`.
    pub const PIPE_FORMAT_B2G3R3_UINT: Format = Format(433);
    /// `PIPE_FORMAT_R4G4B4A4_UINT`.
    pub const PIPE_FORMAT_R4G4B4A4_UINT: Format = Format(434);
    /// `PIPE_FORMAT_B4G4R4A4_UINT`.
    pub const PIPE_FORMAT_B4G4R4A4_UINT: Format = Format(435);
    /// `PIPE_FORMAT_A4R4G4B4_UINT`.
    pub const PIPE_FORMAT_A4R4G4B4_UINT: Format = Format(436);
    /// `PIPE_FORMAT_A4B4G4R4_UINT`.
    pub const PIPE_FORMAT_A4B4G4R4_UINT: Format = Format(437);
    /// `PIPE_FORMAT_A1R5G5B5_UINT`.
    pub const PIPE_FORMAT_A1R5G5B5_UINT: Format = Format(438);
    /// `PIPE_FORMAT_A1B5G5R5_UINT`.
    pub const PIPE_FORMAT_A1B5G5R5_UINT: Format = Format(439);
    /// `PIPE_FORMAT_R5G5B5A1_UINT`.
    pub const PIPE_FORMAT_R5G5B5A1_UINT: Format = Format(440);
    /// `PIPE_FORMAT_B5G5R5A1_UINT`.
    pub const PIPE_FORMAT_B5G5R5A1_UINT: Format = Format(441);
    /// `PIPE_FORMAT_R8G8B8X8_SNORM`.
    pub const PIPE_FORMAT_R8G8B8X8_SNORM: Format = Format(442);
    /// `PIPE_FORMAT_R8G8B8X8_SRGB`.
    pub const PIPE_FORMAT_R8G8B8X8_SRGB: Format = Format(443);
    /// `PIPE_FORMAT_R8G8B8X8_UINT`.
    pub const PIPE_FORMAT_R8G8B8X8_UINT: Format = Format(444);
    /// `PIPE_FORMAT_R8G8B8X8_SINT`.
    pub const PIPE_FORMAT_R8G8B8X8_SINT: Format = Format(445);
    /// `PIPE_FORMAT_B10G10R10X2_UNORM`.
    pub const PIPE_FORMAT_B10G10R10X2_UNORM: Format = Format(446);
    /// `PIPE_FORMAT_B10G10R10X2_SNORM`.
    pub const PIPE_FORMAT_B10G10R10X2_SNORM: Format = Format(447);
    /// `PIPE_FORMAT_B10G10R10X2_SINT`.
    pub const PIPE_FORMAT_B10G10R10X2_SINT: Format = Format(448);
    /// `PIPE_FORMAT_R16G16B16X16_UNORM`.
    pub const PIPE_FORMAT_R16G16B16X16_UNORM: Format = Format(449);
    /// `PIPE_FORMAT_R16G16B16X16_SNORM`.
    pub const PIPE_FORMAT_R16G16B16X16_SNORM: Format = Format(450);
    /// `PIPE_FORMAT_R16G16B16X16_FLOAT`.
    pub const PIPE_FORMAT_R16G16B16X16_FLOAT: Format = Format(451);
    /// `PIPE_FORMAT_R16G16B16X16_UINT`.
    pub const PIPE_FORMAT_R16G16B16X16_UINT: Format = Format(452);
    /// `PIPE_FORMAT_R16G16B16X16_SINT`.
    pub const PIPE_FORMAT_R16G16B16X16_SINT: Format = Format(453);
    /// `PIPE_FORMAT_R32G32B32X32_FLOAT`.
    pub const PIPE_FORMAT_R32G32B32X32_FLOAT: Format = Format(454);
    /// `PIPE_FORMAT_R32G32B32X32_UINT`.
    pub const PIPE_FORMAT_R32G32B32X32_UINT: Format = Format(455);
    /// `PIPE_FORMAT_R32G32B32X32_SINT`.
    pub const PIPE_FORMAT_R32G32B32X32_SINT: Format = Format(456);
    /// `PIPE_FORMAT_R8A8_SNORM`.
    pub const PIPE_FORMAT_R8A8_SNORM: Format = Format(457);
    /// `PIPE_FORMAT_R16A16_UNORM`.
    pub const PIPE_FORMAT_R16A16_UNORM: Format = Format(458);
    /// `PIPE_FORMAT_R16A16_SNORM`.
    pub const PIPE_FORMAT_R16A16_SNORM: Format = Format(459);
    /// `PIPE_FORMAT_R16A16_FLOAT`.
    pub const PIPE_FORMAT_R16A16_FLOAT: Format = Format(460);
    /// `PIPE_FORMAT_R32A32_FLOAT`.
    pub const PIPE_FORMAT_R32A32_FLOAT: Format = Format(461);
    /// `PIPE_FORMAT_R8A8_UINT`.
    pub const PIPE_FORMAT_R8A8_UINT: Format = Format(462);
    /// `PIPE_FORMAT_R8A8_SINT`.
    pub const PIPE_FORMAT_R8A8_SINT: Format = Format(463);
    /// `PIPE_FORMAT_R16A16_UINT`.
    pub const PIPE_FORMAT_R16A16_UINT: Format = Format(464);
    /// `PIPE_FORMAT_R16A16_SINT`.
    pub const PIPE_FORMAT_R16A16_SINT: Format = Format(465);
    /// `PIPE_FORMAT_R32A32_UINT`.
    pub const PIPE_FORMAT_R32A32_UINT: Format = Format(466);
    /// `PIPE_FORMAT_R32A32_SINT`.
    pub const PIPE_FORMAT_R32A32_SINT: Format = Format(467);
    /// `PIPE_FORMAT_B5G6R5_SRGB`.
    pub const PIPE_FORMAT_B5G6R5_SRGB: Format = Format(468);
    /// `PIPE_FORMAT_R5G6B5_SRGB`.
    pub const PIPE_FORMAT_R5G6B5_SRGB: Format = Format(469);
    /// `PIPE_FORMAT_G8R8_UNORM`.
    pub const PIPE_FORMAT_G8R8_UNORM: Format = Format(470);
    /// `PIPE_FORMAT_G8R8_SNORM`.
    pub const PIPE_FORMAT_G8R8_SNORM: Format = Format(471);
    /// `PIPE_FORMAT_G8R8_SINT`.
    pub const PIPE_FORMAT_G8R8_SINT: Format = Format(472);
    /// `PIPE_FORMAT_G16R16_UNORM`.
    pub const PIPE_FORMAT_G16R16_UNORM: Format = Format(473);
    /// `PIPE_FORMAT_G16R16_SNORM`.
    pub const PIPE_FORMAT_G16R16_SNORM: Format = Format(474);
    /// `PIPE_FORMAT_G16R16_SINT`.
    pub const PIPE_FORMAT_G16R16_SINT: Format = Format(475);
    /// `PIPE_FORMAT_A8B8G8R8_SNORM`.
    pub const PIPE_FORMAT_A8B8G8R8_SNORM: Format = Format(476);
    /// `PIPE_FORMAT_A8B8G8R8_SINT`.
    pub const PIPE_FORMAT_A8B8G8R8_SINT: Format = Format(477);
    /// `PIPE_FORMAT_X8B8G8R8_SNORM`.
    pub const PIPE_FORMAT_X8B8G8R8_SNORM: Format = Format(478);
    /// `PIPE_FORMAT_X8B8G8R8_SINT`.
    pub const PIPE_FORMAT_X8B8G8R8_SINT: Format = Format(479);
    /// `PIPE_FORMAT_COUNT`.
    pub const PIPE_FORMAT_COUNT: Format = Format(480);
}

/// Mesa's `pipe_swizzle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Swizzle(pub u32);

impl Swizzle {
    /// `PIPE_SWIZZLE_X`.
    pub const PIPE_SWIZZLE_X: Swizzle = Swizzle(0);
    /// `PIPE_SWIZZLE_Y`.
    pub const PIPE_SWIZZLE_Y: Swizzle = Swizzle(1);
    /// `PIPE_SWIZZLE_Z`.
    pub const PIPE_SWIZZLE_Z: Swizzle = Swizzle(2);
    /// `PIPE_SWIZZLE_W`.
    pub const PIPE_SWIZZLE_W: Swizzle = Swizzle(3);
    /// `PIPE_SWIZZLE_0`.
    pub const PIPE_SWIZZLE_0: Swizzle = Swizzle(4);
    /// `PIPE_SWIZZLE_1`.
    pub const PIPE_SWIZZLE_1: Swizzle = Swizzle(5);
    /// `PIPE_SWIZZLE_NONE`.
    pub const PIPE_SWIZZLE_NONE: Swizzle = Swizzle(6);
    /// `PIPE_SWIZZLE_MAX`.
    pub const PIPE_SWIZZLE_MAX: Swizzle = Swizzle(7);
}

/// Mesa's `pipe_blendfactor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Blendfactor(pub u32);

impl Blendfactor {
    /// `PIPE_BLENDFACTOR_ONE`.
    pub const PIPE_BLENDFACTOR_ONE: Blendfactor = Blendfactor(1);
    /// `PIPE_BLENDFACTOR_SRC_COLOR`.
    pub const PIPE_BLENDFACTOR_SRC_COLOR: Blendfactor = Blendfactor(2);
    /// `PIPE_BLENDFACTOR_SRC_ALPHA`.
    pub const PIPE_BLENDFACTOR_SRC_ALPHA: Blendfactor = Blendfactor(3);
    /// `PIPE_BLENDFACTOR_DST_ALPHA`.
    pub const PIPE_BLENDFACTOR_DST_ALPHA: Blendfactor = Blendfactor(4);
    /// `PIPE_BLENDFACTOR_DST_COLOR`.
    pub const PIPE_BLENDFACTOR_DST_COLOR: Blendfactor = Blendfactor(5);
    /// `PIPE_BLENDFACTOR_SRC_ALPHA_SATURATE`.
    pub const PIPE_BLENDFACTOR_SRC_ALPHA_SATURATE: Blendfactor = Blendfactor(6);
    /// `PIPE_BLENDFACTOR_CONST_COLOR`.
    pub const PIPE_BLENDFACTOR_CONST_COLOR: Blendfactor = Blendfactor(7);
    /// `PIPE_BLENDFACTOR_CONST_ALPHA`.
    pub const PIPE_BLENDFACTOR_CONST_ALPHA: Blendfactor = Blendfactor(8);
    /// `PIPE_BLENDFACTOR_SRC1_COLOR`.
    pub const PIPE_BLENDFACTOR_SRC1_COLOR: Blendfactor = Blendfactor(9);
    /// `PIPE_BLENDFACTOR_SRC1_ALPHA`.
    pub const PIPE_BLENDFACTOR_SRC1_ALPHA: Blendfactor = Blendfactor(10);
    /// `PIPE_BLENDFACTOR_ZERO`.
    pub const PIPE_BLENDFACTOR_ZERO: Blendfactor = Blendfactor(17);
    /// `PIPE_BLENDFACTOR_INV_SRC_COLOR`.
    pub const PIPE_BLENDFACTOR_INV_SRC_COLOR: Blendfactor = Blendfactor(18);
    /// `PIPE_BLENDFACTOR_INV_SRC_ALPHA`.
    pub const PIPE_BLENDFACTOR_INV_SRC_ALPHA: Blendfactor = Blendfactor(19);
    /// `PIPE_BLENDFACTOR_INV_DST_ALPHA`.
    pub const PIPE_BLENDFACTOR_INV_DST_ALPHA: Blendfactor = Blendfactor(20);
    /// `PIPE_BLENDFACTOR_INV_DST_COLOR`.
    pub const PIPE_BLENDFACTOR_INV_DST_COLOR: Blendfactor = Blendfactor(21);
    /// `PIPE_BLENDFACTOR_INV_CONST_COLOR`.
    pub const PIPE_BLENDFACTOR_INV_CONST_COLOR: Blendfactor = Blendfactor(23);
    /// `PIPE_BLENDFACTOR_INV_CONST_ALPHA`.
    pub const PIPE_BLENDFACTOR_INV_CONST_ALPHA: Blendfactor = Blendfactor(24);
    /// `PIPE_BLENDFACTOR_INV_SRC1_COLOR`.
    pub const PIPE_BLENDFACTOR_INV_SRC1_COLOR: Blendfactor = Blendfactor(25);
    /// `PIPE_BLENDFACTOR_INV_SRC1_ALPHA`.
    pub const PIPE_BLENDFACTOR_INV_SRC1_ALPHA: Blendfactor = Blendfactor(26);
}

/// Mesa's `pipe_blend_func`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlendFunc(pub u32);

impl BlendFunc {
    /// `PIPE_BLEND_ADD`.
    pub const PIPE_BLEND_ADD: BlendFunc = BlendFunc(0);
    /// `PIPE_BLEND_SUBTRACT`.
    pub const PIPE_BLEND_SUBTRACT: BlendFunc = BlendFunc(1);
    /// `PIPE_BLEND_REVERSE_SUBTRACT`.
    pub const PIPE_BLEND_REVERSE_SUBTRACT: BlendFunc = BlendFunc(2);
    /// `PIPE_BLEND_MIN`.
    pub const PIPE_BLEND_MIN: BlendFunc = BlendFunc(3);
    /// `PIPE_BLEND_MAX`.
    pub const PIPE_BLEND_MAX: BlendFunc = BlendFunc(4);
}

/// Mesa's `pipe_logicop`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Logicop(pub u32);

impl Logicop {
    /// `PIPE_LOGICOP_CLEAR`.
    pub const PIPE_LOGICOP_CLEAR: Logicop = Logicop(0);
    /// `PIPE_LOGICOP_NOR`.
    pub const PIPE_LOGICOP_NOR: Logicop = Logicop(1);
    /// `PIPE_LOGICOP_AND_INVERTED`.
    pub const PIPE_LOGICOP_AND_INVERTED: Logicop = Logicop(2);
    /// `PIPE_LOGICOP_COPY_INVERTED`.
    pub const PIPE_LOGICOP_COPY_INVERTED: Logicop = Logicop(3);
    /// `PIPE_LOGICOP_AND_REVERSE`.
    pub const PIPE_LOGICOP_AND_REVERSE: Logicop = Logicop(4);
    /// `PIPE_LOGICOP_INVERT`.
    pub const PIPE_LOGICOP_INVERT: Logicop = Logicop(5);
    /// `PIPE_LOGICOP_XOR`.
    pub const PIPE_LOGICOP_XOR: Logicop = Logicop(6);
    /// `PIPE_LOGICOP_NAND`.
    pub const PIPE_LOGICOP_NAND: Logicop = Logicop(7);
    /// `PIPE_LOGICOP_AND`.
    pub const PIPE_LOGICOP_AND: Logicop = Logicop(8);
    /// `PIPE_LOGICOP_EQUIV`.
    pub const PIPE_LOGICOP_EQUIV: Logicop = Logicop(9);
    /// `PIPE_LOGICOP_NOOP`.
    pub const PIPE_LOGICOP_NOOP: Logicop = Logicop(10);
    /// `PIPE_LOGICOP_OR_INVERTED`.
    pub const PIPE_LOGICOP_OR_INVERTED: Logicop = Logicop(11);
    /// `PIPE_LOGICOP_COPY`.
    pub const PIPE_LOGICOP_COPY: Logicop = Logicop(12);
    /// `PIPE_LOGICOP_OR_REVERSE`.
    pub const PIPE_LOGICOP_OR_REVERSE: Logicop = Logicop(13);
    /// `PIPE_LOGICOP_OR`.
    pub const PIPE_LOGICOP_OR: Logicop = Logicop(14);
    /// `PIPE_LOGICOP_SET`.
    pub const PIPE_LOGICOP_SET: Logicop = Logicop(15);
}

/// Mesa's `pipe_advanced_blend_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AdvancedBlendMode(pub u32);

impl AdvancedBlendMode {
    /// `PIPE_ADVANCED_BLEND_NONE`.
    pub const PIPE_ADVANCED_BLEND_NONE: AdvancedBlendMode = AdvancedBlendMode(0);
    /// `PIPE_ADVANCED_BLEND_MULTIPLY`.
    pub const PIPE_ADVANCED_BLEND_MULTIPLY: AdvancedBlendMode = AdvancedBlendMode(1);
    /// `PIPE_ADVANCED_BLEND_SCREEN`.
    pub const PIPE_ADVANCED_BLEND_SCREEN: AdvancedBlendMode = AdvancedBlendMode(2);
    /// `PIPE_ADVANCED_BLEND_OVERLAY`.
    pub const PIPE_ADVANCED_BLEND_OVERLAY: AdvancedBlendMode = AdvancedBlendMode(3);
    /// `PIPE_ADVANCED_BLEND_DARKEN`.
    pub const PIPE_ADVANCED_BLEND_DARKEN: AdvancedBlendMode = AdvancedBlendMode(4);
    /// `PIPE_ADVANCED_BLEND_LIGHTEN`.
    pub const PIPE_ADVANCED_BLEND_LIGHTEN: AdvancedBlendMode = AdvancedBlendMode(5);
    /// `PIPE_ADVANCED_BLEND_COLORDODGE`.
    pub const PIPE_ADVANCED_BLEND_COLORDODGE: AdvancedBlendMode = AdvancedBlendMode(6);
    /// `PIPE_ADVANCED_BLEND_COLORBURN`.
    pub const PIPE_ADVANCED_BLEND_COLORBURN: AdvancedBlendMode = AdvancedBlendMode(7);
    /// `PIPE_ADVANCED_BLEND_HARDLIGHT`.
    pub const PIPE_ADVANCED_BLEND_HARDLIGHT: AdvancedBlendMode = AdvancedBlendMode(8);
    /// `PIPE_ADVANCED_BLEND_SOFTLIGHT`.
    pub const PIPE_ADVANCED_BLEND_SOFTLIGHT: AdvancedBlendMode = AdvancedBlendMode(9);
    /// `PIPE_ADVANCED_BLEND_DIFFERENCE`.
    pub const PIPE_ADVANCED_BLEND_DIFFERENCE: AdvancedBlendMode = AdvancedBlendMode(10);
    /// `PIPE_ADVANCED_BLEND_EXCLUSION`.
    pub const PIPE_ADVANCED_BLEND_EXCLUSION: AdvancedBlendMode = AdvancedBlendMode(11);
    /// `PIPE_ADVANCED_BLEND_HSL_HUE`.
    pub const PIPE_ADVANCED_BLEND_HSL_HUE: AdvancedBlendMode = AdvancedBlendMode(12);
    /// `PIPE_ADVANCED_BLEND_HSL_SATURATION`.
    pub const PIPE_ADVANCED_BLEND_HSL_SATURATION: AdvancedBlendMode = AdvancedBlendMode(13);
    /// `PIPE_ADVANCED_BLEND_HSL_COLOR`.
    pub const PIPE_ADVANCED_BLEND_HSL_COLOR: AdvancedBlendMode = AdvancedBlendMode(14);
    /// `PIPE_ADVANCED_BLEND_HSL_LUMINOSITY`.
    pub const PIPE_ADVANCED_BLEND_HSL_LUMINOSITY: AdvancedBlendMode = AdvancedBlendMode(15);
    /// `PIPE_ADVANCED_BLEND_SRC`.
    pub const PIPE_ADVANCED_BLEND_SRC: AdvancedBlendMode = AdvancedBlendMode(16);
    /// `PIPE_ADVANCED_BLEND_DST`.
    pub const PIPE_ADVANCED_BLEND_DST: AdvancedBlendMode = AdvancedBlendMode(17);
    /// `PIPE_ADVANCED_BLEND_SRC_OVER`.
    pub const PIPE_ADVANCED_BLEND_SRC_OVER: AdvancedBlendMode = AdvancedBlendMode(18);
    /// `PIPE_ADVANCED_BLEND_DST_OVER`.
    pub const PIPE_ADVANCED_BLEND_DST_OVER: AdvancedBlendMode = AdvancedBlendMode(19);
    /// `PIPE_ADVANCED_BLEND_SRC_IN`.
    pub const PIPE_ADVANCED_BLEND_SRC_IN: AdvancedBlendMode = AdvancedBlendMode(20);
    /// `PIPE_ADVANCED_BLEND_DST_IN`.
    pub const PIPE_ADVANCED_BLEND_DST_IN: AdvancedBlendMode = AdvancedBlendMode(21);
    /// `PIPE_ADVANCED_BLEND_SRC_OUT`.
    pub const PIPE_ADVANCED_BLEND_SRC_OUT: AdvancedBlendMode = AdvancedBlendMode(22);
    /// `PIPE_ADVANCED_BLEND_DST_OUT`.
    pub const PIPE_ADVANCED_BLEND_DST_OUT: AdvancedBlendMode = AdvancedBlendMode(23);
    /// `PIPE_ADVANCED_BLEND_SRC_ATOP`.
    pub const PIPE_ADVANCED_BLEND_SRC_ATOP: AdvancedBlendMode = AdvancedBlendMode(24);
    /// `PIPE_ADVANCED_BLEND_DST_ATOP`.
    pub const PIPE_ADVANCED_BLEND_DST_ATOP: AdvancedBlendMode = AdvancedBlendMode(25);
    /// `PIPE_ADVANCED_BLEND_XOR`.
    pub const PIPE_ADVANCED_BLEND_XOR: AdvancedBlendMode = AdvancedBlendMode(26);
    /// `PIPE_ADVANCED_BLEND_INVERT`.
    pub const PIPE_ADVANCED_BLEND_INVERT: AdvancedBlendMode = AdvancedBlendMode(27);
    /// `PIPE_ADVANCED_BLEND_INVERT_RGB`.
    pub const PIPE_ADVANCED_BLEND_INVERT_RGB: AdvancedBlendMode = AdvancedBlendMode(28);
    /// `PIPE_ADVANCED_BLEND_LINEARDODGE`.
    pub const PIPE_ADVANCED_BLEND_LINEARDODGE: AdvancedBlendMode = AdvancedBlendMode(29);
    /// `PIPE_ADVANCED_BLEND_LINEARBURN`.
    pub const PIPE_ADVANCED_BLEND_LINEARBURN: AdvancedBlendMode = AdvancedBlendMode(30);
    /// `PIPE_ADVANCED_BLEND_VIVIDLIGHT`.
    pub const PIPE_ADVANCED_BLEND_VIVIDLIGHT: AdvancedBlendMode = AdvancedBlendMode(31);
    /// `PIPE_ADVANCED_BLEND_LINEARLIGHT`.
    pub const PIPE_ADVANCED_BLEND_LINEARLIGHT: AdvancedBlendMode = AdvancedBlendMode(32);
    /// `PIPE_ADVANCED_BLEND_PINLIGHT`.
    pub const PIPE_ADVANCED_BLEND_PINLIGHT: AdvancedBlendMode = AdvancedBlendMode(33);
    /// `PIPE_ADVANCED_BLEND_HARDMIX`.
    pub const PIPE_ADVANCED_BLEND_HARDMIX: AdvancedBlendMode = AdvancedBlendMode(34);
    /// `PIPE_ADVANCED_BLEND_PLUS`.
    pub const PIPE_ADVANCED_BLEND_PLUS: AdvancedBlendMode = AdvancedBlendMode(35);
    /// `PIPE_ADVANCED_BLEND_PLUS_CLAMPED`.
    pub const PIPE_ADVANCED_BLEND_PLUS_CLAMPED: AdvancedBlendMode = AdvancedBlendMode(36);
    /// `PIPE_ADVANCED_BLEND_PLUS_CLAMPED_ALPHA`.
    pub const PIPE_ADVANCED_BLEND_PLUS_CLAMPED_ALPHA: AdvancedBlendMode = AdvancedBlendMode(37);
    /// `PIPE_ADVANCED_BLEND_PLUS_DARKER`.
    pub const PIPE_ADVANCED_BLEND_PLUS_DARKER: AdvancedBlendMode = AdvancedBlendMode(38);
    /// `PIPE_ADVANCED_BLEND_MINUS`.
    pub const PIPE_ADVANCED_BLEND_MINUS: AdvancedBlendMode = AdvancedBlendMode(39);
    /// `PIPE_ADVANCED_BLEND_MINUS_CLAMPED`.
    pub const PIPE_ADVANCED_BLEND_MINUS_CLAMPED: AdvancedBlendMode = AdvancedBlendMode(40);
    /// `PIPE_ADVANCED_BLEND_CONTRAST`.
    pub const PIPE_ADVANCED_BLEND_CONTRAST: AdvancedBlendMode = AdvancedBlendMode(41);
    /// `PIPE_ADVANCED_BLEND_INVERT_OVG`.
    pub const PIPE_ADVANCED_BLEND_INVERT_OVG: AdvancedBlendMode = AdvancedBlendMode(42);
    /// `PIPE_ADVANCED_BLEND_RED`.
    pub const PIPE_ADVANCED_BLEND_RED: AdvancedBlendMode = AdvancedBlendMode(43);
    /// `PIPE_ADVANCED_BLEND_GREEN`.
    pub const PIPE_ADVANCED_BLEND_GREEN: AdvancedBlendMode = AdvancedBlendMode(44);
    /// `PIPE_ADVANCED_BLEND_BLUE`.
    pub const PIPE_ADVANCED_BLEND_BLUE: AdvancedBlendMode = AdvancedBlendMode(45);
}

/// Mesa's `pipe_blend_overlap_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlendOverlapMode(pub u32);

impl BlendOverlapMode {
    /// `PIPE_BLEND_OVERLAP_UNCORRELATED`.
    pub const PIPE_BLEND_OVERLAP_UNCORRELATED: BlendOverlapMode = BlendOverlapMode(0);
    /// `PIPE_BLEND_OVERLAP_CONJOINT`.
    pub const PIPE_BLEND_OVERLAP_CONJOINT: BlendOverlapMode = BlendOverlapMode(1);
    /// `PIPE_BLEND_OVERLAP_DISJOINT`.
    pub const PIPE_BLEND_OVERLAP_DISJOINT: BlendOverlapMode = BlendOverlapMode(2);
}

/// Mesa's `pipe_error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Error(pub i32);

impl Error {
    /// `PIPE_OK`.
    pub const PIPE_OK: Error = Error(0);
    /// `PIPE_ERROR`.
    pub const PIPE_ERROR: Error = Error(-1);
    /// `PIPE_ERROR_BAD_INPUT`.
    pub const PIPE_ERROR_BAD_INPUT: Error = Error(-2);
    /// `PIPE_ERROR_OUT_OF_MEMORY`.
    pub const PIPE_ERROR_OUT_OF_MEMORY: Error = Error(-3);
    /// `PIPE_ERROR_RETRY`.
    pub const PIPE_ERROR_RETRY: Error = Error(-4);
}

/// Mesa's `pipe_compare_func`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompareFunc(pub u32);

impl CompareFunc {
    /// `PIPE_FUNC_NEVER`.
    pub const PIPE_FUNC_NEVER: CompareFunc = CompareFunc(0);
    /// `PIPE_FUNC_LESS`.
    pub const PIPE_FUNC_LESS: CompareFunc = CompareFunc(1);
    /// `PIPE_FUNC_EQUAL`.
    pub const PIPE_FUNC_EQUAL: CompareFunc = CompareFunc(2);
    /// `PIPE_FUNC_LEQUAL`.
    pub const PIPE_FUNC_LEQUAL: CompareFunc = CompareFunc(3);
    /// `PIPE_FUNC_GREATER`.
    pub const PIPE_FUNC_GREATER: CompareFunc = CompareFunc(4);
    /// `PIPE_FUNC_NOTEQUAL`.
    pub const PIPE_FUNC_NOTEQUAL: CompareFunc = CompareFunc(5);
    /// `PIPE_FUNC_GEQUAL`.
    pub const PIPE_FUNC_GEQUAL: CompareFunc = CompareFunc(6);
    /// `PIPE_FUNC_ALWAYS`.
    pub const PIPE_FUNC_ALWAYS: CompareFunc = CompareFunc(7);
}

/// Mesa's `pipe_stencil_op`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StencilOp(pub u32);

impl StencilOp {
    /// `PIPE_STENCIL_OP_KEEP`.
    pub const PIPE_STENCIL_OP_KEEP: StencilOp = StencilOp(0);
    /// `PIPE_STENCIL_OP_ZERO`.
    pub const PIPE_STENCIL_OP_ZERO: StencilOp = StencilOp(1);
    /// `PIPE_STENCIL_OP_REPLACE`.
    pub const PIPE_STENCIL_OP_REPLACE: StencilOp = StencilOp(2);
    /// `PIPE_STENCIL_OP_INCR`.
    pub const PIPE_STENCIL_OP_INCR: StencilOp = StencilOp(3);
    /// `PIPE_STENCIL_OP_DECR`.
    pub const PIPE_STENCIL_OP_DECR: StencilOp = StencilOp(4);
    /// `PIPE_STENCIL_OP_INCR_WRAP`.
    pub const PIPE_STENCIL_OP_INCR_WRAP: StencilOp = StencilOp(5);
    /// `PIPE_STENCIL_OP_DECR_WRAP`.
    pub const PIPE_STENCIL_OP_DECR_WRAP: StencilOp = StencilOp(6);
    /// `PIPE_STENCIL_OP_INVERT`.
    pub const PIPE_STENCIL_OP_INVERT: StencilOp = StencilOp(7);
}

/// Mesa's `pipe_texture_target`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureTarget(pub u32);

impl TextureTarget {
    /// `PIPE_BUFFER`.
    pub const PIPE_BUFFER: TextureTarget = TextureTarget(0);
    /// `PIPE_TEXTURE_1D`.
    pub const PIPE_TEXTURE_1D: TextureTarget = TextureTarget(1);
    /// `PIPE_TEXTURE_2D`.
    pub const PIPE_TEXTURE_2D: TextureTarget = TextureTarget(2);
    /// `PIPE_TEXTURE_3D`.
    pub const PIPE_TEXTURE_3D: TextureTarget = TextureTarget(3);
    /// `PIPE_TEXTURE_CUBE`.
    pub const PIPE_TEXTURE_CUBE: TextureTarget = TextureTarget(4);
    /// `PIPE_TEXTURE_RECT`.
    pub const PIPE_TEXTURE_RECT: TextureTarget = TextureTarget(5);
    /// `PIPE_TEXTURE_1D_ARRAY`.
    pub const PIPE_TEXTURE_1D_ARRAY: TextureTarget = TextureTarget(6);
    /// `PIPE_TEXTURE_2D_ARRAY`.
    pub const PIPE_TEXTURE_2D_ARRAY: TextureTarget = TextureTarget(7);
    /// `PIPE_TEXTURE_CUBE_ARRAY`.
    pub const PIPE_TEXTURE_CUBE_ARRAY: TextureTarget = TextureTarget(8);
    /// `PIPE_MAX_TEXTURE_TYPES`.
    pub const PIPE_MAX_TEXTURE_TYPES: TextureTarget = TextureTarget(9);
}

/// Mesa's `pipe_tex_face`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TexFace(pub u32);

impl TexFace {
    /// `PIPE_TEX_FACE_POS_X`.
    pub const PIPE_TEX_FACE_POS_X: TexFace = TexFace(0);
    /// `PIPE_TEX_FACE_NEG_X`.
    pub const PIPE_TEX_FACE_NEG_X: TexFace = TexFace(1);
    /// `PIPE_TEX_FACE_POS_Y`.
    pub const PIPE_TEX_FACE_POS_Y: TexFace = TexFace(2);
    /// `PIPE_TEX_FACE_NEG_Y`.
    pub const PIPE_TEX_FACE_NEG_Y: TexFace = TexFace(3);
    /// `PIPE_TEX_FACE_POS_Z`.
    pub const PIPE_TEX_FACE_POS_Z: TexFace = TexFace(4);
    /// `PIPE_TEX_FACE_NEG_Z`.
    pub const PIPE_TEX_FACE_NEG_Z: TexFace = TexFace(5);
    /// `PIPE_TEX_FACE_MAX`.
    pub const PIPE_TEX_FACE_MAX: TexFace = TexFace(6);
}

/// Mesa's `pipe_tex_wrap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TexWrap(pub u32);

impl TexWrap {
    /// `PIPE_TEX_WRAP_REPEAT`.
    pub const PIPE_TEX_WRAP_REPEAT: TexWrap = TexWrap(0);
    /// `PIPE_TEX_WRAP_CLAMP`.
    pub const PIPE_TEX_WRAP_CLAMP: TexWrap = TexWrap(1);
    /// `PIPE_TEX_WRAP_CLAMP_TO_EDGE`.
    pub const PIPE_TEX_WRAP_CLAMP_TO_EDGE: TexWrap = TexWrap(2);
    /// `PIPE_TEX_WRAP_CLAMP_TO_BORDER`.
    pub const PIPE_TEX_WRAP_CLAMP_TO_BORDER: TexWrap = TexWrap(3);
    /// `PIPE_TEX_WRAP_MIRROR_REPEAT`.
    pub const PIPE_TEX_WRAP_MIRROR_REPEAT: TexWrap = TexWrap(4);
    /// `PIPE_TEX_WRAP_MIRROR_CLAMP`.
    pub const PIPE_TEX_WRAP_MIRROR_CLAMP: TexWrap = TexWrap(5);
    /// `PIPE_TEX_WRAP_MIRROR_CLAMP_TO_EDGE`.
    pub const PIPE_TEX_WRAP_MIRROR_CLAMP_TO_EDGE: TexWrap = TexWrap(6);
    /// `PIPE_TEX_WRAP_MIRROR_CLAMP_TO_BORDER`.
    pub const PIPE_TEX_WRAP_MIRROR_CLAMP_TO_BORDER: TexWrap = TexWrap(7);
}

/// Mesa's `pipe_tex_mipfilter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TexMipfilter(pub u32);

impl TexMipfilter {
    /// `PIPE_TEX_MIPFILTER_NEAREST`.
    pub const PIPE_TEX_MIPFILTER_NEAREST: TexMipfilter = TexMipfilter(0);
    /// `PIPE_TEX_MIPFILTER_LINEAR`.
    pub const PIPE_TEX_MIPFILTER_LINEAR: TexMipfilter = TexMipfilter(1);
    /// `PIPE_TEX_MIPFILTER_NONE`.
    pub const PIPE_TEX_MIPFILTER_NONE: TexMipfilter = TexMipfilter(2);
}

/// Mesa's `pipe_tex_filter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TexFilter(pub u32);

impl TexFilter {
    /// `PIPE_TEX_FILTER_NEAREST`.
    pub const PIPE_TEX_FILTER_NEAREST: TexFilter = TexFilter(0);
    /// `PIPE_TEX_FILTER_LINEAR`.
    pub const PIPE_TEX_FILTER_LINEAR: TexFilter = TexFilter(1);
}

/// Mesa's `pipe_tex_compare`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TexCompare(pub u32);

impl TexCompare {
    /// `PIPE_TEX_COMPARE_NONE`.
    pub const PIPE_TEX_COMPARE_NONE: TexCompare = TexCompare(0);
    /// `PIPE_TEX_COMPARE_R_TO_TEXTURE`.
    pub const PIPE_TEX_COMPARE_R_TO_TEXTURE: TexCompare = TexCompare(1);
}

/// Mesa's `pipe_tex_reduction_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TexReductionMode(pub u32);

impl TexReductionMode {
    /// `PIPE_TEX_REDUCTION_WEIGHTED_AVERAGE`.
    pub const PIPE_TEX_REDUCTION_WEIGHTED_AVERAGE: TexReductionMode = TexReductionMode(0);
    /// `PIPE_TEX_REDUCTION_MIN`.
    pub const PIPE_TEX_REDUCTION_MIN: TexReductionMode = TexReductionMode(1);
    /// `PIPE_TEX_REDUCTION_MAX`.
    pub const PIPE_TEX_REDUCTION_MAX: TexReductionMode = TexReductionMode(2);
}

/// Mesa's `pipe_map_flags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MapFlags(pub u32);

impl MapFlags {
    /// `PIPE_MAP_NONE`.
    pub const PIPE_MAP_NONE: MapFlags = MapFlags(0);
    /// `PIPE_MAP_READ`.
    pub const PIPE_MAP_READ: MapFlags = MapFlags(1);
    /// `PIPE_MAP_WRITE`.
    pub const PIPE_MAP_WRITE: MapFlags = MapFlags(2);
    /// `PIPE_MAP_READ_WRITE`.
    pub const PIPE_MAP_READ_WRITE: MapFlags = MapFlags(3);
    /// `PIPE_MAP_DIRECTLY`.
    pub const PIPE_MAP_DIRECTLY: MapFlags = MapFlags(4);
    /// `PIPE_MAP_DISCARD_RANGE`.
    pub const PIPE_MAP_DISCARD_RANGE: MapFlags = MapFlags(8);
    /// `PIPE_MAP_DONTBLOCK`.
    pub const PIPE_MAP_DONTBLOCK: MapFlags = MapFlags(16);
    /// `PIPE_MAP_UNSYNCHRONIZED`.
    pub const PIPE_MAP_UNSYNCHRONIZED: MapFlags = MapFlags(32);
    /// `PIPE_MAP_FLUSH_EXPLICIT`.
    pub const PIPE_MAP_FLUSH_EXPLICIT: MapFlags = MapFlags(64);
    /// `PIPE_MAP_DISCARD_WHOLE_RESOURCE`.
    pub const PIPE_MAP_DISCARD_WHOLE_RESOURCE: MapFlags = MapFlags(128);
    /// `PIPE_MAP_PERSISTENT`.
    pub const PIPE_MAP_PERSISTENT: MapFlags = MapFlags(256);
    /// `PIPE_MAP_COHERENT`.
    pub const PIPE_MAP_COHERENT: MapFlags = MapFlags(512);
    /// `PIPE_MAP_THREAD_SAFE`.
    pub const PIPE_MAP_THREAD_SAFE: MapFlags = MapFlags(1024);
    /// `PIPE_MAP_DEPTH_ONLY`.
    pub const PIPE_MAP_DEPTH_ONLY: MapFlags = MapFlags(2048);
    /// `PIPE_MAP_STENCIL_ONLY`.
    pub const PIPE_MAP_STENCIL_ONLY: MapFlags = MapFlags(4096);
    /// `PIPE_MAP_ONCE`.
    pub const PIPE_MAP_ONCE: MapFlags = MapFlags(8192);
    /// `PIPE_MAP_CLIENT_POINTER`.
    pub const PIPE_MAP_CLIENT_POINTER: MapFlags = MapFlags(16384);
    /// `PIPE_MAP_SHORT_LIVED`.
    pub const PIPE_MAP_SHORT_LIVED: MapFlags = MapFlags(32768);
    /// `PIPE_MAP_DRV_PRV`.
    pub const PIPE_MAP_DRV_PRV: MapFlags = MapFlags(65536);
}

/// Mesa's `pipe_flush_flags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FlushFlags(pub u32);

impl FlushFlags {
    /// `PIPE_FLUSH_END_OF_FRAME`.
    pub const PIPE_FLUSH_END_OF_FRAME: FlushFlags = FlushFlags(1);
    /// `PIPE_FLUSH_DEFERRED`.
    pub const PIPE_FLUSH_DEFERRED: FlushFlags = FlushFlags(2);
    /// `PIPE_FLUSH_FENCE_FD`.
    pub const PIPE_FLUSH_FENCE_FD: FlushFlags = FlushFlags(4);
    /// `PIPE_FLUSH_ASYNC`.
    pub const PIPE_FLUSH_ASYNC: FlushFlags = FlushFlags(8);
    /// `PIPE_FLUSH_HINT_FINISH`.
    pub const PIPE_FLUSH_HINT_FINISH: FlushFlags = FlushFlags(16);
    /// `PIPE_FLUSH_TOP_OF_PIPE`.
    pub const PIPE_FLUSH_TOP_OF_PIPE: FlushFlags = FlushFlags(32);
    /// `PIPE_FLUSH_BOTTOM_OF_PIPE`.
    pub const PIPE_FLUSH_BOTTOM_OF_PIPE: FlushFlags = FlushFlags(64);
    /// `PIPE_FLUSH_INTERMEDIATE`.
    pub const PIPE_FLUSH_INTERMEDIATE: FlushFlags = FlushFlags(128);
}

/// Mesa's `pipe_resource_usage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceUsage(pub u32);

impl ResourceUsage {
    /// `PIPE_USAGE_DEFAULT`.
    pub const PIPE_USAGE_DEFAULT: ResourceUsage = ResourceUsage(0);
    /// `PIPE_USAGE_IMMUTABLE`.
    pub const PIPE_USAGE_IMMUTABLE: ResourceUsage = ResourceUsage(1);
    /// `PIPE_USAGE_DYNAMIC`.
    pub const PIPE_USAGE_DYNAMIC: ResourceUsage = ResourceUsage(2);
    /// `PIPE_USAGE_STREAM`.
    pub const PIPE_USAGE_STREAM: ResourceUsage = ResourceUsage(3);
    /// `PIPE_USAGE_STAGING`.
    pub const PIPE_USAGE_STAGING: ResourceUsage = ResourceUsage(4);
}

/// Mesa's `pipe_tess_spacing`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TessSpacing(pub u32);

impl TessSpacing {
    /// `PIPE_TESS_SPACING_FRACTIONAL_ODD`.
    pub const PIPE_TESS_SPACING_FRACTIONAL_ODD: TessSpacing = TessSpacing(0);
    /// `PIPE_TESS_SPACING_FRACTIONAL_EVEN`.
    pub const PIPE_TESS_SPACING_FRACTIONAL_EVEN: TessSpacing = TessSpacing(1);
    /// `PIPE_TESS_SPACING_EQUAL`.
    pub const PIPE_TESS_SPACING_EQUAL: TessSpacing = TessSpacing(2);
}

/// Mesa's `pipe_query_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueryType(pub u32);

impl QueryType {
    /// `PIPE_QUERY_OCCLUSION_COUNTER`.
    pub const PIPE_QUERY_OCCLUSION_COUNTER: QueryType = QueryType(0);
    /// `PIPE_QUERY_OCCLUSION_PREDICATE`.
    pub const PIPE_QUERY_OCCLUSION_PREDICATE: QueryType = QueryType(1);
    /// `PIPE_QUERY_OCCLUSION_PREDICATE_CONSERVATIVE`.
    pub const PIPE_QUERY_OCCLUSION_PREDICATE_CONSERVATIVE: QueryType = QueryType(2);
    /// `PIPE_QUERY_TIMESTAMP`.
    pub const PIPE_QUERY_TIMESTAMP: QueryType = QueryType(3);
    /// `PIPE_QUERY_TIMESTAMP_DISJOINT`.
    pub const PIPE_QUERY_TIMESTAMP_DISJOINT: QueryType = QueryType(4);
    /// `PIPE_QUERY_TIME_ELAPSED`.
    pub const PIPE_QUERY_TIME_ELAPSED: QueryType = QueryType(5);
    /// `PIPE_QUERY_PRIMITIVES_GENERATED`.
    pub const PIPE_QUERY_PRIMITIVES_GENERATED: QueryType = QueryType(6);
    /// `PIPE_QUERY_PRIMITIVES_EMITTED`.
    pub const PIPE_QUERY_PRIMITIVES_EMITTED: QueryType = QueryType(7);
    /// `PIPE_QUERY_SO_STATISTICS`.
    pub const PIPE_QUERY_SO_STATISTICS: QueryType = QueryType(8);
    /// `PIPE_QUERY_SO_OVERFLOW_PREDICATE`.
    pub const PIPE_QUERY_SO_OVERFLOW_PREDICATE: QueryType = QueryType(9);
    /// `PIPE_QUERY_SO_OVERFLOW_ANY_PREDICATE`.
    pub const PIPE_QUERY_SO_OVERFLOW_ANY_PREDICATE: QueryType = QueryType(10);
    /// `PIPE_QUERY_GPU_FINISHED`.
    pub const PIPE_QUERY_GPU_FINISHED: QueryType = QueryType(11);
    /// `PIPE_QUERY_PIPELINE_STATISTICS`.
    pub const PIPE_QUERY_PIPELINE_STATISTICS: QueryType = QueryType(12);
    /// `PIPE_QUERY_PIPELINE_STATISTICS_SINGLE`.
    pub const PIPE_QUERY_PIPELINE_STATISTICS_SINGLE: QueryType = QueryType(13);
    /// `PIPE_QUERY_TIMESTAMP_RAW`.
    pub const PIPE_QUERY_TIMESTAMP_RAW: QueryType = QueryType(14);
    /// `PIPE_QUERY_TYPES`.
    pub const PIPE_QUERY_TYPES: QueryType = QueryType(15);
    /// `PIPE_QUERY_DRIVER_SPECIFIC`.
    pub const PIPE_QUERY_DRIVER_SPECIFIC: QueryType = QueryType(256);
}

/// Mesa's `pipe_statistics_query_index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StatisticsQueryIndex(pub u32);

impl StatisticsQueryIndex {
    /// `PIPE_STAT_QUERY_IA_VERTICES`.
    pub const PIPE_STAT_QUERY_IA_VERTICES: StatisticsQueryIndex = StatisticsQueryIndex(0);
    /// `PIPE_STAT_QUERY_IA_PRIMITIVES`.
    pub const PIPE_STAT_QUERY_IA_PRIMITIVES: StatisticsQueryIndex = StatisticsQueryIndex(1);
    /// `PIPE_STAT_QUERY_VS_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_VS_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(2);
    /// `PIPE_STAT_QUERY_GS_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_GS_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(3);
    /// `PIPE_STAT_QUERY_GS_PRIMITIVES`.
    pub const PIPE_STAT_QUERY_GS_PRIMITIVES: StatisticsQueryIndex = StatisticsQueryIndex(4);
    /// `PIPE_STAT_QUERY_C_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_C_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(5);
    /// `PIPE_STAT_QUERY_C_PRIMITIVES`.
    pub const PIPE_STAT_QUERY_C_PRIMITIVES: StatisticsQueryIndex = StatisticsQueryIndex(6);
    /// `PIPE_STAT_QUERY_PS_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_PS_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(7);
    /// `PIPE_STAT_QUERY_HS_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_HS_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(8);
    /// `PIPE_STAT_QUERY_DS_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_DS_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(9);
    /// `PIPE_STAT_QUERY_CS_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_CS_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(10);
    /// `PIPE_STAT_QUERY_TS_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_TS_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(11);
    /// `PIPE_STAT_QUERY_MS_INVOCATIONS`.
    pub const PIPE_STAT_QUERY_MS_INVOCATIONS: StatisticsQueryIndex = StatisticsQueryIndex(12);
    /// `PIPE_STAT_QUERY_MS_PRIMITIVES`.
    pub const PIPE_STAT_QUERY_MS_PRIMITIVES: StatisticsQueryIndex = StatisticsQueryIndex(13);
    /// `PIPE_STAT_QUERY_COUNT`.
    pub const PIPE_STAT_QUERY_COUNT: StatisticsQueryIndex = StatisticsQueryIndex(14);
}

/// Mesa's `pipe_render_cond_flag`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderCondFlag(pub u32);

impl RenderCondFlag {
    /// `PIPE_RENDER_COND_WAIT`.
    pub const PIPE_RENDER_COND_WAIT: RenderCondFlag = RenderCondFlag(0);
    /// `PIPE_RENDER_COND_NO_WAIT`.
    pub const PIPE_RENDER_COND_NO_WAIT: RenderCondFlag = RenderCondFlag(1);
    /// `PIPE_RENDER_COND_BY_REGION_WAIT`.
    pub const PIPE_RENDER_COND_BY_REGION_WAIT: RenderCondFlag = RenderCondFlag(2);
    /// `PIPE_RENDER_COND_BY_REGION_NO_WAIT`.
    pub const PIPE_RENDER_COND_BY_REGION_NO_WAIT: RenderCondFlag = RenderCondFlag(3);
}

/// Mesa's `pipe_sprite_coord_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpriteCoordMode(pub u32);

impl SpriteCoordMode {
    /// `PIPE_SPRITE_COORD_UPPER_LEFT`.
    pub const PIPE_SPRITE_COORD_UPPER_LEFT: SpriteCoordMode = SpriteCoordMode(0);
    /// `PIPE_SPRITE_COORD_LOWER_LEFT`.
    pub const PIPE_SPRITE_COORD_LOWER_LEFT: SpriteCoordMode = SpriteCoordMode(1);
}

/// Mesa's `pipe_viewport_swizzle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewportSwizzle(pub u32);

impl ViewportSwizzle {
    /// `PIPE_VIEWPORT_SWIZZLE_POSITIVE_X`.
    pub const PIPE_VIEWPORT_SWIZZLE_POSITIVE_X: ViewportSwizzle = ViewportSwizzle(0);
    /// `PIPE_VIEWPORT_SWIZZLE_NEGATIVE_X`.
    pub const PIPE_VIEWPORT_SWIZZLE_NEGATIVE_X: ViewportSwizzle = ViewportSwizzle(1);
    /// `PIPE_VIEWPORT_SWIZZLE_POSITIVE_Y`.
    pub const PIPE_VIEWPORT_SWIZZLE_POSITIVE_Y: ViewportSwizzle = ViewportSwizzle(2);
    /// `PIPE_VIEWPORT_SWIZZLE_NEGATIVE_Y`.
    pub const PIPE_VIEWPORT_SWIZZLE_NEGATIVE_Y: ViewportSwizzle = ViewportSwizzle(3);
    /// `PIPE_VIEWPORT_SWIZZLE_POSITIVE_Z`.
    pub const PIPE_VIEWPORT_SWIZZLE_POSITIVE_Z: ViewportSwizzle = ViewportSwizzle(4);
    /// `PIPE_VIEWPORT_SWIZZLE_NEGATIVE_Z`.
    pub const PIPE_VIEWPORT_SWIZZLE_NEGATIVE_Z: ViewportSwizzle = ViewportSwizzle(5);
    /// `PIPE_VIEWPORT_SWIZZLE_POSITIVE_W`.
    pub const PIPE_VIEWPORT_SWIZZLE_POSITIVE_W: ViewportSwizzle = ViewportSwizzle(6);
    /// `PIPE_VIEWPORT_SWIZZLE_NEGATIVE_W`.
    pub const PIPE_VIEWPORT_SWIZZLE_NEGATIVE_W: ViewportSwizzle = ViewportSwizzle(7);
}

/// Mesa's `pipe_reset_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResetStatus(pub u32);

impl ResetStatus {
    /// `PIPE_NO_RESET`.
    pub const PIPE_NO_RESET: ResetStatus = ResetStatus(0);
    /// `PIPE_GUILTY_CONTEXT_RESET`.
    pub const PIPE_GUILTY_CONTEXT_RESET: ResetStatus = ResetStatus(1);
    /// `PIPE_INNOCENT_CONTEXT_RESET`.
    pub const PIPE_INNOCENT_CONTEXT_RESET: ResetStatus = ResetStatus(2);
    /// `PIPE_UNKNOWN_CONTEXT_RESET`.
    pub const PIPE_UNKNOWN_CONTEXT_RESET: ResetStatus = ResetStatus(3);
}

/// Mesa's `pipe_vertex_input_alignment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VertexInputAlignment(pub u32);

impl VertexInputAlignment {
    /// `PIPE_VERTEX_INPUT_ALIGNMENT_NONE`.
    pub const PIPE_VERTEX_INPUT_ALIGNMENT_NONE: VertexInputAlignment = VertexInputAlignment(0);
    /// `PIPE_VERTEX_INPUT_ALIGNMENT_4BYTE`.
    pub const PIPE_VERTEX_INPUT_ALIGNMENT_4BYTE: VertexInputAlignment = VertexInputAlignment(1);
    /// `PIPE_VERTEX_INPUT_ALIGNMENT_ELEMENT`.
    pub const PIPE_VERTEX_INPUT_ALIGNMENT_ELEMENT: VertexInputAlignment = VertexInputAlignment(2);
}

/// Mesa's `pipe_conservative_raster_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConservativeRasterMode(pub u32);

impl ConservativeRasterMode {
    /// `PIPE_CONSERVATIVE_RASTER_OFF`.
    pub const PIPE_CONSERVATIVE_RASTER_OFF: ConservativeRasterMode = ConservativeRasterMode(0);
    /// `PIPE_CONSERVATIVE_RASTER_POST_SNAP`.
    pub const PIPE_CONSERVATIVE_RASTER_POST_SNAP: ConservativeRasterMode = ConservativeRasterMode(1);
    /// `PIPE_CONSERVATIVE_RASTER_PRE_SNAP`.
    pub const PIPE_CONSERVATIVE_RASTER_PRE_SNAP: ConservativeRasterMode = ConservativeRasterMode(2);
}

/// Mesa's `pipe_point_size_lower_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PointSizeLowerMode(pub u32);

impl PointSizeLowerMode {
    /// `PIPE_POINT_SIZE_LOWER_ALWAYS`.
    pub const PIPE_POINT_SIZE_LOWER_ALWAYS: PointSizeLowerMode = PointSizeLowerMode(0);
    /// `PIPE_POINT_SIZE_LOWER_NEVER`.
    pub const PIPE_POINT_SIZE_LOWER_NEVER: PointSizeLowerMode = PointSizeLowerMode(1);
    /// `PIPE_POINT_SIZE_LOWER_USER_ONLY`.
    pub const PIPE_POINT_SIZE_LOWER_USER_ONLY: PointSizeLowerMode = PointSizeLowerMode(2);
}

/// Mesa's `pipe_texture_transfer_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureTransferMode(pub u32);

impl TextureTransferMode {
    /// `PIPE_TEXTURE_TRANSFER_DEFAULT`.
    pub const PIPE_TEXTURE_TRANSFER_DEFAULT: TextureTransferMode = TextureTransferMode(0);
    /// `PIPE_TEXTURE_TRANSFER_BLIT`.
    pub const PIPE_TEXTURE_TRANSFER_BLIT: TextureTransferMode = TextureTransferMode(1);
    /// `PIPE_TEXTURE_TRANSFER_COMPUTE`.
    pub const PIPE_TEXTURE_TRANSFER_COMPUTE: TextureTransferMode = TextureTransferMode(2);
}

/// Mesa's `pipe_quirk_texture_border_color_swizzle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QuirkTextureBorderColorSwizzle(pub u32);

impl QuirkTextureBorderColorSwizzle {
    /// `PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_NV50`.
    pub const PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_NV50: QuirkTextureBorderColorSwizzle = QuirkTextureBorderColorSwizzle(1);
    /// `PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_R600`.
    pub const PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_R600: QuirkTextureBorderColorSwizzle = QuirkTextureBorderColorSwizzle(2);
    /// `PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_FREEDRENO`.
    pub const PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_FREEDRENO: QuirkTextureBorderColorSwizzle = QuirkTextureBorderColorSwizzle(4);
    /// `PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_ALPHA_NOT_W`.
    pub const PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_ALPHA_NOT_W: QuirkTextureBorderColorSwizzle = QuirkTextureBorderColorSwizzle(8);
}

/// Mesa's `pipe_device_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceType(pub u32);

impl DeviceType {
    /// `PIPE_DEVICE_TYPE_UNKNOWN`.
    pub const PIPE_DEVICE_TYPE_UNKNOWN: DeviceType = DeviceType(0);
    /// `PIPE_DEVICE_TYPE_INTEGRATED_GPU`.
    pub const PIPE_DEVICE_TYPE_INTEGRATED_GPU: DeviceType = DeviceType(1);
    /// `PIPE_DEVICE_TYPE_DISCRETE_GPU`.
    pub const PIPE_DEVICE_TYPE_DISCRETE_GPU: DeviceType = DeviceType(2);
    /// `PIPE_DEVICE_TYPE_CPU`.
    pub const PIPE_DEVICE_TYPE_CPU: DeviceType = DeviceType(3);
}

/// Mesa's `pipe_endian`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Endian(pub u32);

impl Endian {
    /// `PIPE_ENDIAN_LITTLE`.
    pub const PIPE_ENDIAN_LITTLE: Endian = Endian(0);
    /// `PIPE_ENDIAN_BIG`.
    pub const PIPE_ENDIAN_BIG: Endian = Endian(1);
    /// `PIPE_ENDIAN_NATIVE`.
    pub const PIPE_ENDIAN_NATIVE: Endian = Endian(0);
}

/// Mesa's `pipe_shader_ir`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShaderIr(pub u32);

impl ShaderIr {
    /// `PIPE_SHADER_IR_TGSI`.
    pub const PIPE_SHADER_IR_TGSI: ShaderIr = ShaderIr(0);
    /// `PIPE_SHADER_IR_NIR`.
    pub const PIPE_SHADER_IR_NIR: ShaderIr = ShaderIr(1);
}

/// Mesa's `pipe_resource_param`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceParam(pub u32);

impl ResourceParam {
    /// `PIPE_RESOURCE_PARAM_NPLANES`.
    pub const PIPE_RESOURCE_PARAM_NPLANES: ResourceParam = ResourceParam(0);
    /// `PIPE_RESOURCE_PARAM_STRIDE`.
    pub const PIPE_RESOURCE_PARAM_STRIDE: ResourceParam = ResourceParam(1);
    /// `PIPE_RESOURCE_PARAM_OFFSET`.
    pub const PIPE_RESOURCE_PARAM_OFFSET: ResourceParam = ResourceParam(2);
    /// `PIPE_RESOURCE_PARAM_MODIFIER`.
    pub const PIPE_RESOURCE_PARAM_MODIFIER: ResourceParam = ResourceParam(3);
    /// `PIPE_RESOURCE_PARAM_HANDLE_TYPE_SHARED`.
    pub const PIPE_RESOURCE_PARAM_HANDLE_TYPE_SHARED: ResourceParam = ResourceParam(4);
    /// `PIPE_RESOURCE_PARAM_HANDLE_TYPE_KMS`.
    pub const PIPE_RESOURCE_PARAM_HANDLE_TYPE_KMS: ResourceParam = ResourceParam(5);
    /// `PIPE_RESOURCE_PARAM_HANDLE_TYPE_FD`.
    pub const PIPE_RESOURCE_PARAM_HANDLE_TYPE_FD: ResourceParam = ResourceParam(6);
    /// `PIPE_RESOURCE_PARAM_LAYER_STRIDE`.
    pub const PIPE_RESOURCE_PARAM_LAYER_STRIDE: ResourceParam = ResourceParam(7);
    /// `PIPE_RESOURCE_PARAM_DISJOINT_PLANES`.
    pub const PIPE_RESOURCE_PARAM_DISJOINT_PLANES: ResourceParam = ResourceParam(8);
}

/// Mesa's `pipe_context_param`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContextParam(pub u32);

impl ContextParam {
    /// `PIPE_CONTEXT_PARAM_UPDATE_THREAD_SCHEDULING`.
    pub const PIPE_CONTEXT_PARAM_UPDATE_THREAD_SCHEDULING: ContextParam = ContextParam(0);
    /// `PIPE_CONTEXT_PARAM_MAG_SWITCHOVER_HALF`.
    pub const PIPE_CONTEXT_PARAM_MAG_SWITCHOVER_HALF: ContextParam = ContextParam(1);
}

/// Mesa's `pipe_query_value_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueryValueType(pub u32);

impl QueryValueType {
    /// `PIPE_QUERY_TYPE_I32`.
    pub const PIPE_QUERY_TYPE_I32: QueryValueType = QueryValueType(0);
    /// `PIPE_QUERY_TYPE_U32`.
    pub const PIPE_QUERY_TYPE_U32: QueryValueType = QueryValueType(1);
    /// `PIPE_QUERY_TYPE_I64`.
    pub const PIPE_QUERY_TYPE_I64: QueryValueType = QueryValueType(2);
    /// `PIPE_QUERY_TYPE_U64`.
    pub const PIPE_QUERY_TYPE_U64: QueryValueType = QueryValueType(3);
}

/// Mesa's `pipe_query_flags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QueryFlags(pub u32);

impl QueryFlags {
    /// `PIPE_QUERY_WAIT`.
    pub const PIPE_QUERY_WAIT: QueryFlags = QueryFlags(1);
    /// `PIPE_QUERY_PARTIAL`.
    pub const PIPE_QUERY_PARTIAL: QueryFlags = QueryFlags(2);
}

/// Mesa's `pipe_driver_query_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DriverQueryType(pub u32);

impl DriverQueryType {
    /// `PIPE_DRIVER_QUERY_TYPE_UINT64`.
    pub const PIPE_DRIVER_QUERY_TYPE_UINT64: DriverQueryType = DriverQueryType(0);
    /// `PIPE_DRIVER_QUERY_TYPE_UINT`.
    pub const PIPE_DRIVER_QUERY_TYPE_UINT: DriverQueryType = DriverQueryType(1);
    /// `PIPE_DRIVER_QUERY_TYPE_FLOAT`.
    pub const PIPE_DRIVER_QUERY_TYPE_FLOAT: DriverQueryType = DriverQueryType(2);
    /// `PIPE_DRIVER_QUERY_TYPE_PERCENTAGE`.
    pub const PIPE_DRIVER_QUERY_TYPE_PERCENTAGE: DriverQueryType = DriverQueryType(3);
    /// `PIPE_DRIVER_QUERY_TYPE_BYTES`.
    pub const PIPE_DRIVER_QUERY_TYPE_BYTES: DriverQueryType = DriverQueryType(4);
    /// `PIPE_DRIVER_QUERY_TYPE_MICROSECONDS`.
    pub const PIPE_DRIVER_QUERY_TYPE_MICROSECONDS: DriverQueryType = DriverQueryType(5);
    /// `PIPE_DRIVER_QUERY_TYPE_HZ`.
    pub const PIPE_DRIVER_QUERY_TYPE_HZ: DriverQueryType = DriverQueryType(6);
    /// `PIPE_DRIVER_QUERY_TYPE_DBM`.
    pub const PIPE_DRIVER_QUERY_TYPE_DBM: DriverQueryType = DriverQueryType(7);
    /// `PIPE_DRIVER_QUERY_TYPE_TEMPERATURE`.
    pub const PIPE_DRIVER_QUERY_TYPE_TEMPERATURE: DriverQueryType = DriverQueryType(8);
    /// `PIPE_DRIVER_QUERY_TYPE_VOLTS`.
    pub const PIPE_DRIVER_QUERY_TYPE_VOLTS: DriverQueryType = DriverQueryType(9);
    /// `PIPE_DRIVER_QUERY_TYPE_AMPS`.
    pub const PIPE_DRIVER_QUERY_TYPE_AMPS: DriverQueryType = DriverQueryType(10);
    /// `PIPE_DRIVER_QUERY_TYPE_WATTS`.
    pub const PIPE_DRIVER_QUERY_TYPE_WATTS: DriverQueryType = DriverQueryType(11);
}

/// Mesa's `pipe_driver_query_result_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DriverQueryResultType(pub u32);

impl DriverQueryResultType {
    /// `PIPE_DRIVER_QUERY_RESULT_TYPE_AVERAGE`.
    pub const PIPE_DRIVER_QUERY_RESULT_TYPE_AVERAGE: DriverQueryResultType = DriverQueryResultType(0);
    /// `PIPE_DRIVER_QUERY_RESULT_TYPE_CUMULATIVE`.
    pub const PIPE_DRIVER_QUERY_RESULT_TYPE_CUMULATIVE: DriverQueryResultType = DriverQueryResultType(1);
}

/// Mesa's `pipe_fd_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FdType(pub u32);

impl FdType {
    /// `PIPE_FD_TYPE_NATIVE_SYNC`.
    pub const PIPE_FD_TYPE_NATIVE_SYNC: FdType = FdType(0);
    /// `PIPE_FD_TYPE_SYNCOBJ`.
    pub const PIPE_FD_TYPE_SYNCOBJ: FdType = FdType(1);
    /// `PIPE_FD_TYPE_TIMELINE_SEMAPHORE_D3D12`.
    pub const PIPE_FD_TYPE_TIMELINE_SEMAPHORE_D3D12: FdType = FdType(2);
    /// `PIPE_FD_TYPE_TIMELINE_SEMAPHORE_VK`.
    pub const PIPE_FD_TYPE_TIMELINE_SEMAPHORE_VK: FdType = FdType(3);
}

/// Mesa's `pipe_perf_counter_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PerfCounterType(pub u32);

impl PerfCounterType {
    /// `PIPE_PERF_COUNTER_TYPE_EVENT`.
    pub const PIPE_PERF_COUNTER_TYPE_EVENT: PerfCounterType = PerfCounterType(0);
    /// `PIPE_PERF_COUNTER_TYPE_DURATION_NORM`.
    pub const PIPE_PERF_COUNTER_TYPE_DURATION_NORM: PerfCounterType = PerfCounterType(1);
    /// `PIPE_PERF_COUNTER_TYPE_DURATION_RAW`.
    pub const PIPE_PERF_COUNTER_TYPE_DURATION_RAW: PerfCounterType = PerfCounterType(2);
    /// `PIPE_PERF_COUNTER_TYPE_THROUGHPUT`.
    pub const PIPE_PERF_COUNTER_TYPE_THROUGHPUT: PerfCounterType = PerfCounterType(3);
    /// `PIPE_PERF_COUNTER_TYPE_RAW`.
    pub const PIPE_PERF_COUNTER_TYPE_RAW: PerfCounterType = PerfCounterType(4);
    /// `PIPE_PERF_COUNTER_TYPE_TIMESTAMP`.
    pub const PIPE_PERF_COUNTER_TYPE_TIMESTAMP: PerfCounterType = PerfCounterType(5);
}

/// Mesa's `pipe_perf_counter_data_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PerfCounterDataType(pub u32);

impl PerfCounterDataType {
    /// `PIPE_PERF_COUNTER_DATA_TYPE_BOOL32`.
    pub const PIPE_PERF_COUNTER_DATA_TYPE_BOOL32: PerfCounterDataType = PerfCounterDataType(0);
    /// `PIPE_PERF_COUNTER_DATA_TYPE_UINT32`.
    pub const PIPE_PERF_COUNTER_DATA_TYPE_UINT32: PerfCounterDataType = PerfCounterDataType(1);
    /// `PIPE_PERF_COUNTER_DATA_TYPE_UINT64`.
    pub const PIPE_PERF_COUNTER_DATA_TYPE_UINT64: PerfCounterDataType = PerfCounterDataType(2);
    /// `PIPE_PERF_COUNTER_DATA_TYPE_FLOAT`.
    pub const PIPE_PERF_COUNTER_DATA_TYPE_FLOAT: PerfCounterDataType = PerfCounterDataType(3);
    /// `PIPE_PERF_COUNTER_DATA_TYPE_DOUBLE`.
    pub const PIPE_PERF_COUNTER_DATA_TYPE_DOUBLE: PerfCounterDataType = PerfCounterDataType(4);
}

/// Mesa's `pipe_ml_operation_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MlOperationType(pub u32);

impl MlOperationType {
    /// `PIPE_ML_OPERATION_TYPE_ADD`.
    pub const PIPE_ML_OPERATION_TYPE_ADD: MlOperationType = MlOperationType(0);
    /// `PIPE_ML_OPERATION_TYPE_CONVOLUTION`.
    pub const PIPE_ML_OPERATION_TYPE_CONVOLUTION: MlOperationType = MlOperationType(1);
    /// `PIPE_ML_OPERATION_TYPE_POOLING`.
    pub const PIPE_ML_OPERATION_TYPE_POOLING: MlOperationType = MlOperationType(2);
    /// `PIPE_ML_OPERATION_TYPE_CONCATENATION`.
    pub const PIPE_ML_OPERATION_TYPE_CONCATENATION: MlOperationType = MlOperationType(3);
    /// `PIPE_ML_OPERATION_TYPE_PACK`.
    pub const PIPE_ML_OPERATION_TYPE_PACK: MlOperationType = MlOperationType(4);
    /// `PIPE_ML_OPERATION_TYPE_SPLIT`.
    pub const PIPE_ML_OPERATION_TYPE_SPLIT: MlOperationType = MlOperationType(5);
    /// `PIPE_ML_OPERATION_TYPE_UNPACK`.
    pub const PIPE_ML_OPERATION_TYPE_UNPACK: MlOperationType = MlOperationType(6);
    /// `PIPE_ML_OPERATION_TYPE_PAD`.
    pub const PIPE_ML_OPERATION_TYPE_PAD: MlOperationType = MlOperationType(7);
    /// `PIPE_ML_OPERATION_TYPE_SCATTER_ND`.
    pub const PIPE_ML_OPERATION_TYPE_SCATTER_ND: MlOperationType = MlOperationType(8);
    /// `PIPE_ML_OPERATION_TYPE_FULLY_CONNECTED`.
    pub const PIPE_ML_OPERATION_TYPE_FULLY_CONNECTED: MlOperationType = MlOperationType(9);
    /// `PIPE_ML_OPERATION_TYPE_BATCH_MATMUL`.
    pub const PIPE_ML_OPERATION_TYPE_BATCH_MATMUL: MlOperationType = MlOperationType(10);
    /// `PIPE_ML_OPERATION_TYPE_RESHAPE`.
    pub const PIPE_ML_OPERATION_TYPE_RESHAPE: MlOperationType = MlOperationType(11);
    /// `PIPE_ML_OPERATION_TYPE_RELU`.
    pub const PIPE_ML_OPERATION_TYPE_RELU: MlOperationType = MlOperationType(12);
    /// `PIPE_ML_OPERATION_TYPE_ABSOLUTE`.
    pub const PIPE_ML_OPERATION_TYPE_ABSOLUTE: MlOperationType = MlOperationType(13);
    /// `PIPE_ML_OPERATION_TYPE_LOGISTIC`.
    pub const PIPE_ML_OPERATION_TYPE_LOGISTIC: MlOperationType = MlOperationType(14);
    /// `PIPE_ML_OPERATION_TYPE_TANH`.
    pub const PIPE_ML_OPERATION_TYPE_TANH: MlOperationType = MlOperationType(15);
    /// `PIPE_ML_OPERATION_TYPE_RSQRT`.
    pub const PIPE_ML_OPERATION_TYPE_RSQRT: MlOperationType = MlOperationType(16);
    /// `PIPE_ML_OPERATION_TYPE_HSWISH`.
    pub const PIPE_ML_OPERATION_TYPE_HSWISH: MlOperationType = MlOperationType(17);
    /// `PIPE_ML_OPERATION_TYPE_SUBTRACT`.
    pub const PIPE_ML_OPERATION_TYPE_SUBTRACT: MlOperationType = MlOperationType(18);
    /// `PIPE_ML_OPERATION_TYPE_TRANSPOSE`.
    pub const PIPE_ML_OPERATION_TYPE_TRANSPOSE: MlOperationType = MlOperationType(19);
    /// `PIPE_ML_OPERATION_TYPE_STRIDED_SLICE`.
    pub const PIPE_ML_OPERATION_TYPE_STRIDED_SLICE: MlOperationType = MlOperationType(20);
    /// `PIPE_ML_OPERATION_TYPE_RESIZE`.
    pub const PIPE_ML_OPERATION_TYPE_RESIZE: MlOperationType = MlOperationType(21);
    /// `PIPE_ML_OPERATION_TYPE_RESIZE_BILINEAR`.
    pub const PIPE_ML_OPERATION_TYPE_RESIZE_BILINEAR: MlOperationType = MlOperationType(22);
    /// `PIPE_ML_OPERATION_TYPE_ARGMAX`.
    pub const PIPE_ML_OPERATION_TYPE_ARGMAX: MlOperationType = MlOperationType(23);
    /// `PIPE_ML_OPERATION_TYPE_SPACE_TO_BATCH`.
    pub const PIPE_ML_OPERATION_TYPE_SPACE_TO_BATCH: MlOperationType = MlOperationType(24);
    /// `PIPE_ML_OPERATION_TYPE_BATCH_TO_SPACE`.
    pub const PIPE_ML_OPERATION_TYPE_BATCH_TO_SPACE: MlOperationType = MlOperationType(25);
    /// `PIPE_ML_OPERATION_TYPE_MUL`.
    pub const PIPE_ML_OPERATION_TYPE_MUL: MlOperationType = MlOperationType(26);
    /// `PIPE_ML_OPERATION_TYPE_LEAKY_RELU`.
    pub const PIPE_ML_OPERATION_TYPE_LEAKY_RELU: MlOperationType = MlOperationType(27);
    /// `PIPE_ML_OPERATION_TYPE_QUANTIZE`.
    pub const PIPE_ML_OPERATION_TYPE_QUANTIZE: MlOperationType = MlOperationType(28);
    /// `PIPE_ML_OPERATION_TYPE_MAXIMUM`.
    pub const PIPE_ML_OPERATION_TYPE_MAXIMUM: MlOperationType = MlOperationType(29);
    /// `PIPE_ML_OPERATION_TYPE_MINIMUM`.
    pub const PIPE_ML_OPERATION_TYPE_MINIMUM: MlOperationType = MlOperationType(30);
    /// `PIPE_ML_OPERATION_TYPE_SOFTMAX`.
    pub const PIPE_ML_OPERATION_TYPE_SOFTMAX: MlOperationType = MlOperationType(31);
    /// `PIPE_ML_OPERATION_TYPE_MEAN`.
    pub const PIPE_ML_OPERATION_TYPE_MEAN: MlOperationType = MlOperationType(32);
}

/// Mesa's `pipe_ml_pooling_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MlPoolingType(pub u32);

impl MlPoolingType {
    /// `PIPE_ML_POOLING_TYPE_AVG`.
    pub const PIPE_ML_POOLING_TYPE_AVG: MlPoolingType = MlPoolingType(0);
    /// `PIPE_ML_POOLING_TYPE_MAX`.
    pub const PIPE_ML_POOLING_TYPE_MAX: MlPoolingType = MlPoolingType(1);
}

/// `PIPE_POLYGON_MODE_FILL`.
pub const PIPE_POLYGON_MODE_FILL: u32 = 0;
/// `PIPE_POLYGON_MODE_LINE`.
pub const PIPE_POLYGON_MODE_LINE: u32 = 1;
/// `PIPE_POLYGON_MODE_POINT`.
pub const PIPE_POLYGON_MODE_POINT: u32 = 2;
/// `PIPE_POLYGON_MODE_FILL_RECTANGLE`.
pub const PIPE_POLYGON_MODE_FILL_RECTANGLE: u32 = 3;
/// `PIPE_ASTC_DECODE_FORMAT_FLOAT16`.
pub const PIPE_ASTC_DECODE_FORMAT_FLOAT16: u32 = 0;
/// `PIPE_ASTC_DECODE_FORMAT_RGB9E5`.
pub const PIPE_ASTC_DECODE_FORMAT_RGB9E5: u32 = 2;
/// `PIPE_ASTC_DECODE_FORMAT_UNORM8`.
pub const PIPE_ASTC_DECODE_FORMAT_UNORM8: u32 = 1;
/// `PIPE_BARRIER_ALL`.
pub const PIPE_BARRIER_ALL: u32 = 8191;
/// `PIPE_BARRIER_CONSTANT_BUFFER`.
pub const PIPE_BARRIER_CONSTANT_BUFFER: u32 = 32;
/// `PIPE_BARRIER_FRAMEBUFFER`.
pub const PIPE_BARRIER_FRAMEBUFFER: u32 = 512;
/// `PIPE_BARRIER_IMAGE`.
pub const PIPE_BARRIER_IMAGE: u32 = 256;
/// `PIPE_BARRIER_INDEX_BUFFER`.
pub const PIPE_BARRIER_INDEX_BUFFER: u32 = 16;
/// `PIPE_BARRIER_INDIRECT_BUFFER`.
pub const PIPE_BARRIER_INDIRECT_BUFFER: u32 = 64;
/// `PIPE_BARRIER_MAPPED_BUFFER`.
pub const PIPE_BARRIER_MAPPED_BUFFER: u32 = 1;
/// `PIPE_BARRIER_QUERY_BUFFER`.
pub const PIPE_BARRIER_QUERY_BUFFER: u32 = 4;
/// `PIPE_BARRIER_SHADER_BUFFER`.
pub const PIPE_BARRIER_SHADER_BUFFER: u32 = 2;
/// `PIPE_BARRIER_STREAMOUT_BUFFER`.
pub const PIPE_BARRIER_STREAMOUT_BUFFER: u32 = 1024;
/// `PIPE_BARRIER_TEXTURE`.
pub const PIPE_BARRIER_TEXTURE: u32 = 128;
/// `PIPE_BARRIER_UPDATE`.
pub const PIPE_BARRIER_UPDATE: u32 = 6144;
/// `PIPE_BARRIER_UPDATE_BUFFER`.
pub const PIPE_BARRIER_UPDATE_BUFFER: u32 = 2048;
/// `PIPE_BARRIER_UPDATE_TEXTURE`.
pub const PIPE_BARRIER_UPDATE_TEXTURE: u32 = 4096;
/// `PIPE_BARRIER_VERTEX_BUFFER`.
pub const PIPE_BARRIER_VERTEX_BUFFER: u32 = 8;
/// `PIPE_BIND_BLENDABLE`.
pub const PIPE_BIND_BLENDABLE: u32 = 4;
/// `PIPE_BIND_COMMAND_ARGS_BUFFER`.
pub const PIPE_BIND_COMMAND_ARGS_BUFFER: u32 = 131072;
/// `PIPE_BIND_CONSTANT_BUFFER`.
pub const PIPE_BIND_CONSTANT_BUFFER: u32 = 64;
/// `PIPE_BIND_CONST_BW`.
pub const PIPE_BIND_CONST_BW: u32 = 67108864;
/// `PIPE_BIND_CURSOR`.
pub const PIPE_BIND_CURSOR: u32 = 2048;
/// `PIPE_BIND_CUSTOM`.
pub const PIPE_BIND_CUSTOM: u32 = 4096;
/// `PIPE_BIND_DEPTH_STENCIL`.
pub const PIPE_BIND_DEPTH_STENCIL: u32 = 1;
/// `PIPE_BIND_DISPLAY_TARGET`.
pub const PIPE_BIND_DISPLAY_TARGET: u32 = 128;
/// `PIPE_BIND_GLOBAL`.
pub const PIPE_BIND_GLOBAL: u32 = 8192;
/// `PIPE_BIND_INDEX_BUFFER`.
pub const PIPE_BIND_INDEX_BUFFER: u32 = 32;
/// `PIPE_BIND_LINEAR`.
pub const PIPE_BIND_LINEAR: u32 = 2097152;
/// `PIPE_BIND_OPENCL`.
pub const PIPE_BIND_OPENCL: u32 = 65536;
/// `PIPE_BIND_PRIME_BLIT_DST`.
pub const PIPE_BIND_PRIME_BLIT_DST: u32 = 16777216;
/// `PIPE_BIND_PROTECTED`.
pub const PIPE_BIND_PROTECTED: u32 = 4194304;
/// `PIPE_BIND_QUERY_BUFFER`.
pub const PIPE_BIND_QUERY_BUFFER: u32 = 262144;
/// `PIPE_BIND_RENDER_TARGET`.
pub const PIPE_BIND_RENDER_TARGET: u32 = 2;
/// `PIPE_BIND_SAMPLER_REDUCTION_MINMAX`.
pub const PIPE_BIND_SAMPLER_REDUCTION_MINMAX: u32 = 8388608;
/// `PIPE_BIND_SAMPLER_VIEW`.
pub const PIPE_BIND_SAMPLER_VIEW: u32 = 8;
/// `PIPE_BIND_SAMPLER_VIEW_SUBOPTIMAL`.
pub const PIPE_BIND_SAMPLER_VIEW_SUBOPTIMAL: u32 = 512;
/// `PIPE_BIND_SCANOUT`.
pub const PIPE_BIND_SCANOUT: u32 = 524288;
/// `PIPE_BIND_SHADER_BUFFER`.
pub const PIPE_BIND_SHADER_BUFFER: u32 = 16384;
/// `PIPE_BIND_SHADER_IMAGE`.
pub const PIPE_BIND_SHADER_IMAGE: u32 = 32768;
/// `PIPE_BIND_SHARED`.
pub const PIPE_BIND_SHARED: u32 = 1048576;
/// `PIPE_BIND_STREAM_OUTPUT`.
pub const PIPE_BIND_STREAM_OUTPUT: u32 = 1024;
/// `PIPE_BIND_USE_FRONT_RENDERING`.
pub const PIPE_BIND_USE_FRONT_RENDERING: u32 = 33554432;
/// `PIPE_BIND_VERTEX_BUFFER`.
pub const PIPE_BIND_VERTEX_BUFFER: u32 = 16;
/// `PIPE_BIND_VERTEX_STATE`.
pub const PIPE_BIND_VERTEX_STATE: u32 = 256;
/// `PIPE_BIND_VIDEO_DECODE_DPB`.
pub const PIPE_BIND_VIDEO_DECODE_DPB: u32 = 134217728;
/// `PIPE_BIND_VIDEO_ENCODE_DPB`.
pub const PIPE_BIND_VIDEO_ENCODE_DPB: u32 = 268435456;
/// `PIPE_BLENDFACTOR_INVERT_BIT`.
pub const PIPE_BLENDFACTOR_INVERT_BIT: u32 = 16;
/// `PIPE_CLEAR_COLOR`.
pub const PIPE_CLEAR_COLOR: u32 = 1020;
/// `PIPE_CLEAR_COLOR0`.
pub const PIPE_CLEAR_COLOR0: u32 = 4;
/// `PIPE_CLEAR_COLOR1`.
pub const PIPE_CLEAR_COLOR1: u32 = 8;
/// `PIPE_CLEAR_COLOR2`.
pub const PIPE_CLEAR_COLOR2: u32 = 16;
/// `PIPE_CLEAR_COLOR3`.
pub const PIPE_CLEAR_COLOR3: u32 = 32;
/// `PIPE_CLEAR_COLOR4`.
pub const PIPE_CLEAR_COLOR4: u32 = 64;
/// `PIPE_CLEAR_COLOR5`.
pub const PIPE_CLEAR_COLOR5: u32 = 128;
/// `PIPE_CLEAR_COLOR6`.
pub const PIPE_CLEAR_COLOR6: u32 = 256;
/// `PIPE_CLEAR_COLOR7`.
pub const PIPE_CLEAR_COLOR7: u32 = 512;
/// `PIPE_CLEAR_DEPTH`.
pub const PIPE_CLEAR_DEPTH: u32 = 1;
/// `PIPE_CLEAR_DEPTHSTENCIL`.
pub const PIPE_CLEAR_DEPTHSTENCIL: u32 = 3;
/// `PIPE_CLEAR_STENCIL`.
pub const PIPE_CLEAR_STENCIL: u32 = 2;
/// `PIPE_COMPRESSION_FIXED_RATE_DEFAULT`.
pub const PIPE_COMPRESSION_FIXED_RATE_DEFAULT: u32 = 15;
/// `PIPE_COMPRESSION_FIXED_RATE_NONE`.
pub const PIPE_COMPRESSION_FIXED_RATE_NONE: u32 = 0;
/// `PIPE_CONTEXT_COMPUTE_ONLY`.
pub const PIPE_CONTEXT_COMPUTE_ONLY: u32 = 1;
/// `PIPE_CONTEXT_DEBUG`.
pub const PIPE_CONTEXT_DEBUG: u32 = 2;
/// `PIPE_CONTEXT_HIGH_PRIORITY`.
pub const PIPE_CONTEXT_HIGH_PRIORITY: u32 = 16;
/// `PIPE_CONTEXT_LOSE_CONTEXT_ON_RESET`.
pub const PIPE_CONTEXT_LOSE_CONTEXT_ON_RESET: u32 = 64;
/// `PIPE_CONTEXT_LOW_PRIORITY`.
pub const PIPE_CONTEXT_LOW_PRIORITY: u32 = 32;
/// `PIPE_CONTEXT_MEDIA_ONLY`.
pub const PIPE_CONTEXT_MEDIA_ONLY: u32 = 512;
/// `PIPE_CONTEXT_NO_LOD_BIAS`.
pub const PIPE_CONTEXT_NO_LOD_BIAS: u32 = 256;
/// `PIPE_CONTEXT_PREFER_THREADED`.
pub const PIPE_CONTEXT_PREFER_THREADED: u32 = 8;
/// `PIPE_CONTEXT_PRIORITY_HIGH`.
pub const PIPE_CONTEXT_PRIORITY_HIGH: u32 = 4;
/// `PIPE_CONTEXT_PRIORITY_LOW`.
pub const PIPE_CONTEXT_PRIORITY_LOW: u32 = 1;
/// `PIPE_CONTEXT_PRIORITY_MEDIUM`.
pub const PIPE_CONTEXT_PRIORITY_MEDIUM: u32 = 2;
/// `PIPE_CONTEXT_PRIORITY_REALTIME`.
pub const PIPE_CONTEXT_PRIORITY_REALTIME: u32 = 8;
/// `PIPE_CONTEXT_PROTECTED`.
pub const PIPE_CONTEXT_PROTECTED: u32 = 128;
/// `PIPE_CONTEXT_REALTIME_PRIORITY`.
pub const PIPE_CONTEXT_REALTIME_PRIORITY: u32 = 1024;
/// `PIPE_CONTEXT_ROBUST_BUFFER_ACCESS`.
pub const PIPE_CONTEXT_ROBUST_BUFFER_ACCESS: u32 = 4;
/// `PIPE_DRIVER_QUERY_FLAG_BATCH`.
pub const PIPE_DRIVER_QUERY_FLAG_BATCH: u32 = 1;
/// `PIPE_DRIVER_QUERY_FLAG_DONT_LIST`.
pub const PIPE_DRIVER_QUERY_FLAG_DONT_LIST: u32 = 2;
/// `PIPE_DUMP_DEVICE_STATUS_REGISTERS`.
pub const PIPE_DUMP_DEVICE_STATUS_REGISTERS: u32 = 1;
/// `PIPE_FACE_BACK`.
pub const PIPE_FACE_BACK: u32 = 2;
/// `PIPE_FACE_FRONT`.
pub const PIPE_FACE_FRONT: u32 = 1;
/// `PIPE_FACE_FRONT_AND_BACK`.
pub const PIPE_FACE_FRONT_AND_BACK: u32 = 3;
/// `PIPE_FACE_NONE`.
pub const PIPE_FACE_NONE: u32 = 0;
/// `PIPE_FORMAT_ABGR8888_SNORM`.
pub const PIPE_FORMAT_ABGR8888_SNORM: u32 = 476;
/// `PIPE_FORMAT_ABGR8888_SRGB`.
pub const PIPE_FORMAT_ABGR8888_SRGB: u32 = 347;
/// `PIPE_FORMAT_ABGR8888_UINT`.
pub const PIPE_FORMAT_ABGR8888_UINT: u32 = 427;
/// `PIPE_FORMAT_ABGR8888_UNORM`.
pub const PIPE_FORMAT_ABGR8888_UNORM: u32 = 291;
/// `PIPE_FORMAT_ARGB8888_SRGB`.
pub const PIPE_FORMAT_ARGB8888_SRGB: u32 = 351;
/// `PIPE_FORMAT_ARGB8888_UINT`.
pub const PIPE_FORMAT_ARGB8888_UINT: u32 = 426;
/// `PIPE_FORMAT_ARGB8888_UNORM`.
pub const PIPE_FORMAT_ARGB8888_UNORM: u32 = 111;
/// `PIPE_FORMAT_AYUV`.
pub const PIPE_FORMAT_AYUV: u32 = 129;
/// `PIPE_FORMAT_B8G8_R8G8_UNORM`.
pub const PIPE_FORMAT_B8G8_R8G8_UNORM: u32 = 364;
/// `PIPE_FORMAT_B8R8_G8R8_UNORM`.
pub const PIPE_FORMAT_B8R8_G8R8_UNORM: u32 = 361;
/// `PIPE_FORMAT_BGRA8888_SRGB`.
pub const PIPE_FORMAT_BGRA8888_SRGB: u32 = 349;
/// `PIPE_FORMAT_BGRA8888_UINT`.
pub const PIPE_FORMAT_BGRA8888_UINT: u32 = 103;
/// `PIPE_FORMAT_BGRA8888_UNORM`.
pub const PIPE_FORMAT_BGRA8888_UNORM: u32 = 105;
/// `PIPE_FORMAT_BGRX8888_SRGB`.
pub const PIPE_FORMAT_BGRX8888_SRGB: u32 = 350;
/// `PIPE_FORMAT_BGRX8888_UNORM`.
pub const PIPE_FORMAT_BGRX8888_UNORM: u32 = 282;
/// `PIPE_FORMAT_G8B8_G8R8_UNORM`.
pub const PIPE_FORMAT_G8B8_G8R8_UNORM: u32 = 363;
/// `PIPE_FORMAT_G8R8_B8R8_UNORM`.
pub const PIPE_FORMAT_G8R8_B8R8_UNORM: u32 = 359;
/// `PIPE_FORMAT_G8R8_G8B8_UNORM`.
pub const PIPE_FORMAT_G8R8_G8B8_UNORM: u32 = 358;
/// `PIPE_FORMAT_GR1616_SNORM`.
pub const PIPE_FORMAT_GR1616_SNORM: u32 = 474;
/// `PIPE_FORMAT_GR1616_UNORM`.
pub const PIPE_FORMAT_GR1616_UNORM: u32 = 473;
/// `PIPE_FORMAT_GR88_SNORM`.
pub const PIPE_FORMAT_GR88_SNORM: u32 = 471;
/// `PIPE_FORMAT_GR88_UNORM`.
pub const PIPE_FORMAT_GR88_UNORM: u32 = 470;
/// `PIPE_FORMAT_IYUV`.
pub const PIPE_FORMAT_IYUV: u32 = 133;
/// `PIPE_FORMAT_NV12`.
pub const PIPE_FORMAT_NV12: u32 = 134;
/// `PIPE_FORMAT_NV15`.
pub const PIPE_FORMAT_NV15: u32 = 137;
/// `PIPE_FORMAT_NV16`.
pub const PIPE_FORMAT_NV16: u32 = 141;
/// `PIPE_FORMAT_NV20`.
pub const PIPE_FORMAT_NV20: u32 = 138;
/// `PIPE_FORMAT_NV21`.
pub const PIPE_FORMAT_NV21: u32 = 135;
/// `PIPE_FORMAT_NV24`.
pub const PIPE_FORMAT_NV24: u32 = 143;
/// `PIPE_FORMAT_NV42`.
pub const PIPE_FORMAT_NV42: u32 = 144;
/// `PIPE_FORMAT_NV61`.
pub const PIPE_FORMAT_NV61: u32 = 142;
/// `PIPE_FORMAT_P010`.
pub const PIPE_FORMAT_P010: u32 = 157;
/// `PIPE_FORMAT_P012`.
pub const PIPE_FORMAT_P012: u32 = 158;
/// `PIPE_FORMAT_P016`.
pub const PIPE_FORMAT_P016: u32 = 159;
/// `PIPE_FORMAT_P030`.
pub const PIPE_FORMAT_P030: u32 = 161;
/// `PIPE_FORMAT_P210`.
pub const PIPE_FORMAT_P210: u32 = 160;
/// `PIPE_FORMAT_R8B8_R8G8_UNORM`.
pub const PIPE_FORMAT_R8B8_R8G8_UNORM: u32 = 362;
/// `PIPE_FORMAT_R8G8_B8G8_UNORM`.
pub const PIPE_FORMAT_R8G8_B8G8_UNORM: u32 = 357;
/// `PIPE_FORMAT_R8G8_R8B8_UNORM`.
pub const PIPE_FORMAT_R8G8_R8B8_UNORM: u32 = 360;
/// `PIPE_FORMAT_R8_G8_B8_UNORM`.
pub const PIPE_FORMAT_R8_G8_B8_UNORM: u32 = 384;
/// `PIPE_FORMAT_RG1616_SNORM`.
pub const PIPE_FORMAT_RG1616_SNORM: u32 = 62;
/// `PIPE_FORMAT_RG1616_UNORM`.
pub const PIPE_FORMAT_RG1616_UNORM: u32 = 58;
/// `PIPE_FORMAT_RG88_SNORM`.
pub const PIPE_FORMAT_RG88_SNORM: u32 = 86;
/// `PIPE_FORMAT_RG88_UNORM`.
pub const PIPE_FORMAT_RG88_UNORM: u32 = 82;
/// `PIPE_FORMAT_RGBA8888_SINT`.
pub const PIPE_FORMAT_RGBA8888_SINT: u32 = 80;
/// `PIPE_FORMAT_RGBA8888_SNORM`.
pub const PIPE_FORMAT_RGBA8888_SNORM: u32 = 88;
/// `PIPE_FORMAT_RGBA8888_SRGB`.
pub const PIPE_FORMAT_RGBA8888_SRGB: u32 = 346;
/// `PIPE_FORMAT_RGBA8888_SSCALED`.
pub const PIPE_FORMAT_RGBA8888_SSCALED: u32 = 96;
/// `PIPE_FORMAT_RGBA8888_UINT`.
pub const PIPE_FORMAT_RGBA8888_UINT: u32 = 76;
/// `PIPE_FORMAT_RGBA8888_UNORM`.
pub const PIPE_FORMAT_RGBA8888_UNORM: u32 = 84;
/// `PIPE_FORMAT_RGBA8888_USCALED`.
pub const PIPE_FORMAT_RGBA8888_USCALED: u32 = 92;
/// `PIPE_FORMAT_RGBX8888_SNORM`.
pub const PIPE_FORMAT_RGBX8888_SNORM: u32 = 442;
/// `PIPE_FORMAT_RGBX8888_SRGB`.
pub const PIPE_FORMAT_RGBX8888_SRGB: u32 = 443;
/// `PIPE_FORMAT_RGBX8888_UNORM`.
pub const PIPE_FORMAT_RGBX8888_UNORM: u32 = 293;
/// `PIPE_FORMAT_UYVY`.
pub const PIPE_FORMAT_UYVY: u32 = 125;
/// `PIPE_FORMAT_VYUY`.
pub const PIPE_FORMAT_VYUY: u32 = 126;
/// `PIPE_FORMAT_XBGR8888_SNORM`.
pub const PIPE_FORMAT_XBGR8888_SNORM: u32 = 478;
/// `PIPE_FORMAT_XBGR8888_SRGB`.
pub const PIPE_FORMAT_XBGR8888_SRGB: u32 = 348;
/// `PIPE_FORMAT_XBGR8888_UNORM`.
pub const PIPE_FORMAT_XBGR8888_UNORM: u32 = 292;
/// `PIPE_FORMAT_XRGB8888_SRGB`.
pub const PIPE_FORMAT_XRGB8888_SRGB: u32 = 352;
/// `PIPE_FORMAT_XRGB8888_UNORM`.
pub const PIPE_FORMAT_XRGB8888_UNORM: u32 = 288;
/// `PIPE_FORMAT_XYUV`.
pub const PIPE_FORMAT_XYUV: u32 = 130;
/// `PIPE_FORMAT_Y210`.
pub const PIPE_FORMAT_Y210: u32 = 162;
/// `PIPE_FORMAT_Y212`.
pub const PIPE_FORMAT_Y212: u32 = 163;
/// `PIPE_FORMAT_Y216`.
pub const PIPE_FORMAT_Y216: u32 = 164;
/// `PIPE_FORMAT_Y410`.
pub const PIPE_FORMAT_Y410: u32 = 165;
/// `PIPE_FORMAT_Y412`.
pub const PIPE_FORMAT_Y412: u32 = 166;
/// `PIPE_FORMAT_Y416`.
pub const PIPE_FORMAT_Y416: u32 = 167;
/// `PIPE_FORMAT_Y8_UNORM`.
pub const PIPE_FORMAT_Y8_UNORM: u32 = 136;
/// `PIPE_FORMAT_YUYV`.
pub const PIPE_FORMAT_YUYV: u32 = 127;
/// `PIPE_FORMAT_YV12`.
pub const PIPE_FORMAT_YV12: u32 = 131;
/// `PIPE_FORMAT_YV16`.
pub const PIPE_FORMAT_YV16: u32 = 132;
/// `PIPE_FORMAT_YVYU`.
pub const PIPE_FORMAT_YVYU: u32 = 128;
/// `PIPE_HANDLE_USAGE_EXPLICIT_FLUSH`.
pub const PIPE_HANDLE_USAGE_EXPLICIT_FLUSH: u32 = 1;
/// `PIPE_HANDLE_USAGE_FRAMEBUFFER_WRITE`.
pub const PIPE_HANDLE_USAGE_FRAMEBUFFER_WRITE: u32 = 2;
/// `PIPE_HANDLE_USAGE_SHADER_WRITE`.
pub const PIPE_HANDLE_USAGE_SHADER_WRITE: u32 = 4;
/// `PIPE_IMAGE_ACCESS_COHERENT`.
pub const PIPE_IMAGE_ACCESS_COHERENT: u32 = 4;
/// `PIPE_IMAGE_ACCESS_DRIVER_INTERNAL`.
pub const PIPE_IMAGE_ACCESS_DRIVER_INTERNAL: u32 = 32;
/// `PIPE_IMAGE_ACCESS_READ`.
pub const PIPE_IMAGE_ACCESS_READ: u32 = 1;
/// `PIPE_IMAGE_ACCESS_READ_WRITE`.
pub const PIPE_IMAGE_ACCESS_READ_WRITE: u32 = 3;
/// `PIPE_IMAGE_ACCESS_TEX2D_FROM_BUFFER`.
pub const PIPE_IMAGE_ACCESS_TEX2D_FROM_BUFFER: u32 = 16;
/// `PIPE_IMAGE_ACCESS_VOLATILE`.
pub const PIPE_IMAGE_ACCESS_VOLATILE: u32 = 8;
/// `PIPE_IMAGE_ACCESS_WRITE`.
pub const PIPE_IMAGE_ACCESS_WRITE: u32 = 2;
/// `PIPE_LUID_SIZE`.
pub const PIPE_LUID_SIZE: u32 = 8;
/// `PIPE_MASK_A`.
pub const PIPE_MASK_A: u32 = 8;
/// `PIPE_MASK_B`.
pub const PIPE_MASK_B: u32 = 4;
/// `PIPE_MASK_G`.
pub const PIPE_MASK_G: u32 = 2;
/// `PIPE_MASK_R`.
pub const PIPE_MASK_R: u32 = 1;
/// `PIPE_MASK_RGB`.
pub const PIPE_MASK_RGB: u32 = 7;
/// `PIPE_MASK_RGBA`.
pub const PIPE_MASK_RGBA: u32 = 15;
/// `PIPE_MASK_RGBAZS`.
pub const PIPE_MASK_RGBAZS: u32 = 63;
/// `PIPE_MASK_S`.
pub const PIPE_MASK_S: u32 = 32;
/// `PIPE_MASK_Z`.
pub const PIPE_MASK_Z: u32 = 16;
/// `PIPE_MASK_ZS`.
pub const PIPE_MASK_ZS: u32 = 48;
/// `PIPE_MAX_ATTRIBS`.
pub const PIPE_MAX_ATTRIBS: u32 = 32;
/// `PIPE_MAX_CLIP_OR_CULL_DISTANCE_COUNT`.
pub const PIPE_MAX_CLIP_OR_CULL_DISTANCE_COUNT: u32 = 8;
/// `PIPE_MAX_CLIP_OR_CULL_DISTANCE_ELEMENT_COUNT`.
pub const PIPE_MAX_CLIP_OR_CULL_DISTANCE_ELEMENT_COUNT: u32 = 2;
/// `PIPE_MAX_CLIP_PLANES`.
pub const PIPE_MAX_CLIP_PLANES: u32 = 8;
/// `PIPE_MAX_COLOR_BUFS`.
pub const PIPE_MAX_COLOR_BUFS: u32 = 8;
/// `PIPE_MAX_CONSTANT_BUFFERS`.
pub const PIPE_MAX_CONSTANT_BUFFERS: u32 = 32;
/// `PIPE_MAX_HW_ATOMIC_BUFFERS`.
pub const PIPE_MAX_HW_ATOMIC_BUFFERS: u32 = 32;
/// `PIPE_MAX_SAMPLERS`.
pub const PIPE_MAX_SAMPLERS: u32 = 32;
/// `PIPE_MAX_SAMPLE_LOCATION_GRID_SIZE`.
pub const PIPE_MAX_SAMPLE_LOCATION_GRID_SIZE: u32 = 4;
/// `PIPE_MAX_SHADER_BUFFERS`.
pub const PIPE_MAX_SHADER_BUFFERS: u32 = 32;
/// `PIPE_MAX_SHADER_IMAGES`.
pub const PIPE_MAX_SHADER_IMAGES: u32 = 64;
/// `PIPE_MAX_SHADER_INPUTS`.
pub const PIPE_MAX_SHADER_INPUTS: u32 = 80;
/// `PIPE_MAX_SHADER_OUTPUTS`.
pub const PIPE_MAX_SHADER_OUTPUTS: u32 = 80;
/// `PIPE_MAX_SHADER_SAMPLER_VIEWS`.
pub const PIPE_MAX_SHADER_SAMPLER_VIEWS: u32 = 128;
/// `PIPE_MAX_SO_BUFFERS`.
pub const PIPE_MAX_SO_BUFFERS: u32 = 4;
/// `PIPE_MAX_SO_OUTPUTS`.
pub const PIPE_MAX_SO_OUTPUTS: u32 = 128;
/// `PIPE_MAX_TEXTURE_LEVELS`.
pub const PIPE_MAX_TEXTURE_LEVELS: u32 = 16;
/// `PIPE_MAX_VERTEX_STREAMS`.
pub const PIPE_MAX_VERTEX_STREAMS: u32 = 4;
/// `PIPE_MAX_VIEWPORTS`.
pub const PIPE_MAX_VIEWPORTS: u32 = 16;
/// `PIPE_MAX_WINDOW_RECTANGLES`.
pub const PIPE_MAX_WINDOW_RECTANGLES: u32 = 8;
/// `PIPE_RESOURCE_FLAG_DONT_MAP_DIRECTLY`.
pub const PIPE_RESOURCE_FLAG_DONT_MAP_DIRECTLY: u32 = 128;
/// `PIPE_RESOURCE_FLAG_DONT_OVER_ALLOCATE`.
pub const PIPE_RESOURCE_FLAG_DONT_OVER_ALLOCATE: u32 = 64;
/// `PIPE_RESOURCE_FLAG_DRV_PRIV`.
pub const PIPE_RESOURCE_FLAG_DRV_PRIV: u32 = 8192;
/// `PIPE_RESOURCE_FLAG_ENCRYPTED`.
pub const PIPE_RESOURCE_FLAG_ENCRYPTED: u32 = 32;
/// `PIPE_RESOURCE_FLAG_FIXED_ADDRESS`.
pub const PIPE_RESOURCE_FLAG_FIXED_ADDRESS: u32 = 512;
/// `PIPE_RESOURCE_FLAG_FRONTEND_PRIV`.
pub const PIPE_RESOURCE_FLAG_FRONTEND_PRIV: u32 = 16777216;
/// `PIPE_RESOURCE_FLAG_FRONTEND_VM`.
pub const PIPE_RESOURCE_FLAG_FRONTEND_VM: u32 = 1024;
/// `PIPE_RESOURCE_FLAG_MAP_CLIENT_POINTER`.
pub const PIPE_RESOURCE_FLAG_MAP_CLIENT_POINTER: u32 = 4096;
/// `PIPE_RESOURCE_FLAG_MAP_COHERENT`.
pub const PIPE_RESOURCE_FLAG_MAP_COHERENT: u32 = 2;
/// `PIPE_RESOURCE_FLAG_MAP_PERSISTENT`.
pub const PIPE_RESOURCE_FLAG_MAP_PERSISTENT: u32 = 1;
/// `PIPE_RESOURCE_FLAG_MAP_UNSYNCHRONIZED`.
pub const PIPE_RESOURCE_FLAG_MAP_UNSYNCHRONIZED: u32 = 2048;
/// `PIPE_RESOURCE_FLAG_SINGLE_THREAD_USE`.
pub const PIPE_RESOURCE_FLAG_SINGLE_THREAD_USE: u32 = 16;
/// `PIPE_RESOURCE_FLAG_SPARSE`.
pub const PIPE_RESOURCE_FLAG_SPARSE: u32 = 8;
/// `PIPE_RESOURCE_FLAG_TEXTURING_MORE_LIKELY`.
pub const PIPE_RESOURCE_FLAG_TEXTURING_MORE_LIKELY: u32 = 4;
/// `PIPE_RESOURCE_FLAG_UNMAPPABLE`.
pub const PIPE_RESOURCE_FLAG_UNMAPPABLE: u32 = 256;
/// `PIPE_SHADER_SUBGROUP_FEATURE_ARITHMETIC`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_ARITHMETIC: u32 = 4;
/// `PIPE_SHADER_SUBGROUP_FEATURE_BALLOT`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_BALLOT: u32 = 8;
/// `PIPE_SHADER_SUBGROUP_FEATURE_BASIC`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_BASIC: u32 = 1;
/// `PIPE_SHADER_SUBGROUP_FEATURE_CLUSTERED`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_CLUSTERED: u32 = 64;
/// `PIPE_SHADER_SUBGROUP_FEATURE_QUAD`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_QUAD: u32 = 128;
/// `PIPE_SHADER_SUBGROUP_FEATURE_ROTATE`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_ROTATE: u32 = 512;
/// `PIPE_SHADER_SUBGROUP_FEATURE_ROTATE_CLUSTERED`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_ROTATE_CLUSTERED: u32 = 1024;
/// `PIPE_SHADER_SUBGROUP_FEATURE_SHUFFLE`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_SHUFFLE: u32 = 16;
/// `PIPE_SHADER_SUBGROUP_FEATURE_SHUFFLE_RELATIVE`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_SHUFFLE_RELATIVE: u32 = 32;
/// `PIPE_SHADER_SUBGROUP_FEATURE_VOTE`.
pub const PIPE_SHADER_SUBGROUP_FEATURE_VOTE: u32 = 2;
/// `PIPE_SHADER_SUBGROUP_NUM_FEATURES`.
pub const PIPE_SHADER_SUBGROUP_NUM_FEATURES: u32 = 8;
/// `PIPE_TEXTURE_BARRIER_FRAMEBUFFER`.
pub const PIPE_TEXTURE_BARRIER_FRAMEBUFFER: u32 = 2;
/// `PIPE_TEXTURE_BARRIER_SAMPLER`.
pub const PIPE_TEXTURE_BARRIER_SAMPLER: u32 = 1;
/// `PIPE_UUID_SIZE`.
pub const PIPE_UUID_SIZE: u32 = 16;
