//! Apple's Metal on one GPU as a safe trait: buffers, textures, compiled functions, pipelines and command buffers.

use core::fmt::Debug;

use raw_window_handle::HasWindowHandle;

/// One Metal device and its command queue, whose objects are handles that a clone names again.
///
/// Every call is checked before it reaches Metal, and every function is compiled with its accesses bounds-checked, so no
/// command lets the GPU reach memory outside its resources.
pub trait Metal {
    /// A buffer in memory the CPU and the GPU share.
    type Buffer: Clone + Debug;
    /// A texture in memory only the GPU reaches, or a view of one.
    type Texture: Clone + Debug;
    type SamplerState: Clone + Debug;
    type DepthStencilState: Clone + Debug;
    /// One entry point compiled for one stage, with the slots it reads.
    type Function: Clone + Debug;
    type RenderPipelineState: Clone + Debug;
    type ComputePipelineState: Clone + Debug;
    /// A render pipeline Metal builds on a thread of its own, which `finish_render_pipeline_state` waits for.
    type PendingRenderPipelineState: Clone + Debug;
    type CommandBuffer: Clone + Debug;
    /// The Core Animation layer of a window's view, whose drawables a window shows.
    type Layer: Clone + Debug;
    /// One picture of a layer: a texture to draw into, shown once presented.
    type Drawable: Clone + Debug;
    /// Places the GPU writes its clock into, in nanoseconds.
    type TimestampBuffer: Clone + Debug;

    /// Returns the GPU's name, as Metal gives it.
    fn name(&self) -> String;
    /// Returns the most bytes one buffer may hold, past which `make_buffer` refuses.
    fn max_buffer_length(&self) -> u64;
    /// Returns whether passes take [`RenderCommand::SetDepthTestBounds`], which Metal has from macOS 26.
    ///
    /// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/setdepthtestbounds(_:)
    fn supports_depth_test_bounds(&self) -> bool;
    /// Makes a buffer of `length` bytes, all zero.
    fn make_buffer(&mut self, length: u64) -> Result<Self::Buffer, MakeBufferError>;
    /// Copies `bytes` into the buffer at `offset`, after every committed command buffer that uses it has completed.
    fn write_contents(
        &mut self,
        buffer: &Self::Buffer,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), WriteContentsError>;
    /// Copies the buffer's bytes from `offset` into `bytes`, after every committed command buffer that uses it has completed.
    fn read_contents(
        &mut self,
        buffer: &Self::Buffer,
        offset: u64,
        bytes: &mut [u8],
    ) -> Result<(), ReadContentsError>;
    /// Makes a texture with undefined texels.
    fn make_texture(
        &mut self,
        descriptor: &TextureDescriptor,
    ) -> Result<Self::Texture, MakeTextureError>;
    /// Makes a texture that sees levels and slices of another in another format of the same texel size.
    fn make_texture_view(
        &mut self,
        texture: &Self::Texture,
        descriptor: &TextureViewDescriptor,
    ) -> Result<Self::Texture, MakeTextureViewError>;
    fn make_sampler_state(
        &mut self,
        descriptor: &SamplerDescriptor,
    ) -> Result<Self::SamplerState, MakeSamplerStateError>;
    fn make_depth_stencil_state(
        &mut self,
        descriptor: &DepthStencilDescriptor,
    ) -> Result<Self::DepthStencilState, MakeDepthStencilStateError>;
    /// Writes one entry point of a module as Metal Shading Language with every access bounds-checked, and compiles it.
    fn make_function(
        &mut self,
        source: &FunctionSource<'_>,
    ) -> Result<Self::Function, MakeFunctionError>;
    fn make_render_pipeline_state(
        &mut self,
        descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> Result<Self::RenderPipelineState, MakeRenderPipelineStateError>;
    /// Makes a function as `make_function` does, every check made now, but compiled by Metal on a thread of its own.
    ///
    /// A pipeline made of it waits for the compile, and a compile Metal refuses refuses that pipeline.
    ///
    /// https://developer.apple.com/documentation/metal/mtldevice/makelibrary(source:options:completionhandler:)
    fn make_function_in_background(
        &mut self,
        source: &FunctionSource<'_>,
    ) -> Result<Self::Function, MakeFunctionError>;
    /// Makes a render pipeline as `make_render_pipeline_state` does, every check made now, but built in the background.
    ///
    /// Metal builds it on a thread of its own once its functions are compiled.
    ///
    /// https://developer.apple.com/documentation/metal/mtldevice/makerenderpipelinestate(descriptor:completionhandler:)
    fn make_render_pipeline_state_in_background(
        &mut self,
        descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> Result<Self::PendingRenderPipelineState, MakeRenderPipelineStateError>;
    /// Waits for a pipeline built in the background and returns it, or why Metal refused it.
    fn finish_render_pipeline_state(
        &mut self,
        pending: &Self::PendingRenderPipelineState,
    ) -> Result<Self::RenderPipelineState, MakeRenderPipelineStateError>;
    /// Makes a pipeline that runs a compute function, its threadgroup the size the function declares.
    fn make_compute_pipeline_state(
        &mut self,
        function: &Self::Function,
    ) -> Result<Self::ComputePipelineState, MakeComputePipelineStateError>;
    /// Makes a command buffer that keeps every resource encoded into it until it completes.
    fn make_command_buffer(&mut self) -> Result<Self::CommandBuffer, MakeCommandBufferError>;
    /// Encodes one render pass of `commands` into the command buffer, checking each draw against what is bound.
    fn encode_render_commands(
        &mut self,
        command_buffer: &Self::CommandBuffer,
        pass: &RenderPassDescriptor<'_, Self>,
        commands: &[RenderCommand<Self>],
    ) -> Result<(), EncodeRenderCommandsError>;
    /// Encodes copies and fills into the command buffer, in order.
    fn encode_blit_commands(
        &mut self,
        command_buffer: &Self::CommandBuffer,
        commands: &[BlitCommand<Self>],
    ) -> Result<(), EncodeBlitCommandsError>;
    /// Encodes dispatches into the command buffer, checking each against what is bound.
    fn encode_compute_commands(
        &mut self,
        command_buffer: &Self::CommandBuffer,
        commands: &[ComputeCommand<Self>],
    ) -> Result<(), EncodeComputeCommandsError>;
    /// Sends the command buffer to the GPU, after every command buffer committed before it.
    fn commit(&mut self, command_buffer: &Self::CommandBuffer) -> Result<(), CommitError>;
    /// Blocks the calling thread until the committed command buffer has completed.
    fn wait_until_completed(
        &mut self,
        command_buffer: &Self::CommandBuffer,
    ) -> Result<(), WaitUntilCompletedError>;
    /// Returns how far the GPU has come with the command buffer.
    fn status(&self, command_buffer: &Self::CommandBuffer) -> CommandBufferStatus;
    /// Returns the Metal layer of a window's view, made there if it has none; called on the main thread.
    fn make_layer(&mut self, window: &dyn HasWindowHandle) -> Result<Self::Layer, MakeLayerError>;
    /// Sets the format and size of the layer's drawables, and whether presenting waits for the display.
    fn configure_layer(
        &mut self,
        layer: &Self::Layer,
        descriptor: &LayerDescriptor,
    ) -> Result<(), ConfigureLayerError>;
    /// Returns the layer's next drawable, waiting until one is free.
    fn next_drawable(&mut self, layer: &Self::Layer) -> Result<Self::Drawable, NextDrawableError>;
    /// Returns the texture a drawable shows, which passes draw into and nothing else reads.
    fn drawable_texture(&self, drawable: &Self::Drawable) -> Self::Texture;
    /// Shows the drawable once the command buffer has completed.
    fn present_drawable(
        &mut self,
        command_buffer: &Self::CommandBuffer,
        drawable: &Self::Drawable,
    ) -> Result<(), PresentDrawableError>;
    /// Makes `sample_count` places for timestamps.
    fn make_timestamp_buffer(
        &mut self,
        sample_count: u32,
    ) -> Result<Self::TimestampBuffer, MakeTimestampBufferError>;
    /// Encodes a pass of its own that writes the GPU's clock into place `index` once the work encoded before it ends.
    fn encode_timestamp(
        &mut self,
        command_buffer: &Self::CommandBuffer,
        timestamps: &Self::TimestampBuffer,
        index: u32,
    ) -> Result<(), EncodeTimestampError>;
    /// Copies the timestamps of places from `first` on into `into`.
    ///
    /// It waits until every committed command buffer that writes them has completed.
    fn read_timestamps(
        &mut self,
        timestamps: &Self::TimestampBuffer,
        first: u32,
        into: &mut [u64],
    ) -> Result<(), ReadTimestampsError>;
}

/// The most places of one timestamp buffer, WebGPU's most queries of a set.
///
/// https://www.w3.org/TR/webgpu/#dom-gpudevice-createqueryset
pub const MAX_TIMESTAMPS: u32 = 4096;

/// Why a timestamp buffer was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeTimestampBufferError {
    /// No places, or more than `MAX_TIMESTAMPS`.
    Count,
    /// The GPU writes no timestamps where passes end.
    Unsupported,
}

/// Why timestamps were not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadTimestampsError {
    /// Places past the buffer's last.
    OutOfRange,
    /// A command buffer not committed writes them, so they would never be written.
    UsedByUncommittedWork,
    /// Metal gave no values.
    Refused,
}

/// Why a timestamp was not encoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodeTimestampError {
    /// The command buffer was already committed.
    Committed,
    /// A place past the buffer's last.
    OutOfRange,
    /// Metal made no pass for it.
    Refused,
}

/// Why a buffer was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeBufferError {
    /// Longer than the device's longest buffer.
    TooLong,
    /// Metal made none.
    Refused,
}

/// Why bytes were not written into a buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteContentsError {
    OutOfRange,
    /// A command buffer not yet committed uses the buffer, so waiting for it would never end.
    UsedByUncommittedWork,
}

/// Why bytes were not read from a buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadContentsError {
    OutOfRange,
    /// A command buffer not yet committed uses the buffer, so waiting for it would never end.
    UsedByUncommittedWork,
}

/// A texture to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureDescriptor {
    pub texture_type: TextureType,
    pub pixel_format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mipmap_level_count: u32,
    pub sample_count: u32,
    /// Layers of an array type, cubes of a cube array; one otherwise.
    pub array_length: u32,
    pub usage: TextureUsage,
}

/// The shape of a texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureType {
    Type2D,
    Type2DArray,
    Type2DMultisample,
    TypeCube,
    TypeCubeArray,
    Type3D,
}

/// The layout of texels in a texture, by Metal's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    A8Unorm,
    R8Unorm,
    R8UnormSrgb,
    R8Snorm,
    R8Uint,
    R8Sint,
    R16Unorm,
    R16Snorm,
    R16Uint,
    R16Sint,
    R16Float,
    Rg8Unorm,
    Rg8Snorm,
    Rg8Uint,
    Rg8Sint,
    B5g6r5Unorm,
    A1bgr5Unorm,
    Abgr4Unorm,
    Bgr5a1Unorm,
    R32Uint,
    R32Sint,
    R32Float,
    Rg16Unorm,
    Rg16Snorm,
    Rg16Uint,
    Rg16Sint,
    Rg16Float,
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Rgba8Snorm,
    Rgba8Uint,
    Rgba8Sint,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Rgb10a2Unorm,
    Rgb10a2Uint,
    Rg11b10Float,
    Rgb9e5Float,
    Rg32Uint,
    Rg32Sint,
    Rg32Float,
    Rgba16Unorm,
    Rgba16Snorm,
    Rgba16Uint,
    Rgba16Sint,
    Rgba16Float,
    Rgba32Uint,
    Rgba32Sint,
    Rgba32Float,
    Bc1Rgba,
    Bc1RgbaSrgb,
    Bc2Rgba,
    Bc2RgbaSrgb,
    Bc3Rgba,
    Bc3RgbaSrgb,
    Bc4RUnorm,
    Bc4RSnorm,
    Bc5RgUnorm,
    Bc5RgSnorm,
    Bc6hRgbFloat,
    Bc6hRgbUfloat,
    Bc7RgbaUnorm,
    Bc7RgbaUnormSrgb,
    Etc2Rgb8,
    Etc2Rgb8Srgb,
    EacRgba8,
    EacRgba8Srgb,
    Astc4x4Ldr,
    Astc4x4Srgb,
    Depth16Unorm,
    Depth32Float,
    Stencil8,
    Depth32FloatStencil8,
    X32Stencil8,
}

/// The ways a texture is used besides copies, which every texture takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureUsage {
    pub shader_read: bool,
    pub shader_write: bool,
    pub render_target: bool,
    /// Views of it may take another format of the same texel size.
    pub pixel_format_view: bool,
}

/// Why a texture was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeTextureError {
    /// A size, level count, sample count or array length the type and format do not take.
    Invalid(&'static str),
    Refused,
}

/// The part of a texture a view sees, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureViewDescriptor {
    pub pixel_format: PixelFormat,
    pub texture_type: TextureType,
    pub first_level: u32,
    pub level_count: u32,
    /// The first slice: a layer, or a face of a cube counted across every cube.
    pub first_slice: u32,
    pub slice_count: u32,
    /// What each channel the view's reads give holds.
    ///
    /// Other than [`TextureSwizzleChannels::IDENTITY`] only on a texture not made to be drawn into, and not of a view
    /// with one.
    pub swizzle: TextureSwizzleChannels,
}

/// What one channel of a view's reads holds: one of the texel's channels, or a constant.
///
/// https://developer.apple.com/documentation/metal/mtltextureswizzle
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureSwizzle {
    Zero,
    One,
    Red,
    Green,
    Blue,
    Alpha,
}

/// What the red, green, blue and alpha channels of a view's reads hold.
///
/// https://developer.apple.com/documentation/metal/mtltextureswizzlechannels
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureSwizzleChannels {
    pub red: TextureSwizzle,
    pub green: TextureSwizzle,
    pub blue: TextureSwizzle,
    pub alpha: TextureSwizzle,
}

impl TextureSwizzleChannels {
    /// Each channel its own.
    pub const IDENTITY: TextureSwizzleChannels = TextureSwizzleChannels {
        red: TextureSwizzle::Red,
        green: TextureSwizzle::Green,
        blue: TextureSwizzle::Blue,
        alpha: TextureSwizzle::Alpha,
    };
}

/// Why a texture view was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeTextureViewError {
    /// Levels or slices outside the texture.
    OutOfRange,
    /// A format or type the texture cannot be seen as.
    Incompatible,
    Refused,
}

/// A sampler state to make.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerDescriptor {
    pub min_filter: SamplerMinMagFilter,
    pub mag_filter: SamplerMinMagFilter,
    pub mip_filter: SamplerMipFilter,
    pub s_address_mode: SamplerAddressMode,
    pub t_address_mode: SamplerAddressMode,
    pub r_address_mode: SamplerAddressMode,
    pub border_color: SamplerBorderColor,
    pub lod_min_clamp: f32,
    pub lod_max_clamp: f32,
    /// One to 16.
    pub max_anisotropy: u32,
    /// The comparison of a comparison sampler, which a function reads depth through.
    pub compare_function: Option<CompareFunction>,
}

/// How a sampler filters between texels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerMinMagFilter {
    Nearest,
    Linear,
}

/// How a sampler filters between levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerMipFilter {
    NotMipmapped,
    Nearest,
    Linear,
}

/// What a sampler reads outside the texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerAddressMode {
    ClampToEdge,
    MirrorClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToZero,
    ClampToBorderColor,
}

/// The color `SamplerAddressMode::ClampToBorderColor` reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerBorderColor {
    TransparentBlack,
    OpaqueBlack,
    OpaqueWhite,
}

/// A comparison of a new value, on the left, with a stored one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// Why a sampler state was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeSamplerStateError {
    Invalid(&'static str),
    Refused,
}

/// A depth and stencil state to make; no stencil descriptor keeps the stencil and passes its test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DepthStencilDescriptor {
    pub depth_compare_function: CompareFunction,
    pub depth_write_enabled: bool,
    pub front_face_stencil: Option<StencilDescriptor>,
    pub back_face_stencil: Option<StencilDescriptor>,
}

/// One face's stencil test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StencilDescriptor {
    pub stencil_compare_function: CompareFunction,
    pub stencil_failure_operation: StencilOperation,
    pub depth_failure_operation: StencilOperation,
    pub depth_stencil_pass_operation: StencilOperation,
    pub read_mask: u32,
    pub write_mask: u32,
}

/// What a stencil test does with the stored value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StencilOperation {
    Keep,
    Zero,
    Replace,
    IncrementClamp,
    DecrementClamp,
    Invert,
    IncrementWrap,
    DecrementWrap,
}

/// Why a depth and stencil state was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeDepthStencilStateError {
    Refused,
}

/// One entry point of a validated module, and where its resources come from.
#[derive(Clone, Copy, Debug)]
pub struct FunctionSource<'a> {
    pub module: &'a naga::Module,
    pub info: &'a naga::valid::ModuleInfo,
    pub stage: naga::ShaderStage,
    pub entry_point: &'a str,
    /// A slot for every resource binding the entry point reads.
    pub resources: &'a [ResourceSlot],
    /// The buffer slot of the module's immediates, if it has any.
    pub immediates_buffer_index: u32,
    /// The buffer slot the lengths of bound buffers are passed in, which the bounds checks read.
    pub sizes_buffer_index: u32,
    /// The buffers a vertex function reads its inputs from.
    pub vertex_buffers: &'a [VertexBufferLayout],
    /// The function draws points, whose size it writes.
    pub point_size: bool,
}

/// Which slot one resource binding of a module is read through: a buffer, texture or sampler slot by its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResourceSlot {
    pub group: u32,
    pub binding: u32,
    pub index: u32,
}

/// A vertex buffer a vertex function reads its inputs from, bounds-checked by the function itself.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VertexBufferLayout {
    pub buffer_index: u32,
    pub stride: u32,
    pub step_function: VertexStepFunction,
    /// The instances that read one element, from the base instance, where the buffer advances `PerInstance`.
    ///
    /// Zero reads the base instance's element for every instance. Read only for `PerInstance`.
    ///
    /// https://developer.apple.com/documentation/metal/mtlvertexbufferlayoutdescriptor/steprate
    pub step_rate: u32,
    pub attributes: Vec<VertexAttribute>,
}

/// How a vertex buffer advances.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexStepFunction {
    /// Every vertex and instance reads the first element.
    Constant,
    PerVertex,
    PerInstance,
}

/// One attribute of a vertex buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexAttribute {
    /// The vertex input it feeds.
    pub location: u32,
    pub offset: u32,
    pub format: VertexFormat,
}

/// How a vertex attribute is stored, by Metal's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexFormat {
    UChar2,
    UChar4,
    Char2,
    Char4,
    UChar2Normalized,
    UChar4Normalized,
    Char2Normalized,
    Char4Normalized,
    UShort2,
    UShort4,
    Short2,
    Short4,
    UShort2Normalized,
    UShort4Normalized,
    Short2Normalized,
    Short4Normalized,
    Half2,
    Half4,
    Float,
    Float2,
    Float3,
    Float4,
    UInt,
    UInt2,
    UInt3,
    UInt4,
    Int,
    Int2,
    Int3,
    Int4,
    UChar,
    Char,
    UCharNormalized,
    CharNormalized,
    UShort,
    Short,
    UShortNormalized,
    ShortNormalized,
    Half,
    UInt1010102Normalized,
    UChar4NormalizedBgra,
}

/// Why a function was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeFunctionError {
    /// A stage, binding or type this crate does not check, such as a texture the function writes.
    Unchecked(&'static str),
    /// The module could not be written as Metal Shading Language.
    Translation(String),
    /// Metal's compiler refused the written function.
    Compilation(String),
}

/// A render pipeline to make.
#[derive(Debug)]
pub struct RenderPipelineDescriptor<'a, M: Metal + ?Sized> {
    pub vertex_function: &'a M::Function,
    pub fragment_function: Option<&'a M::Function>,
    pub color_attachments: [Option<RenderPipelineColorAttachmentDescriptor>; MAX_COLOR_ATTACHMENTS],
    pub depth_attachment_pixel_format: Option<PixelFormat>,
    pub stencil_attachment_pixel_format: Option<PixelFormat>,
    pub raster_sample_count: u32,
    pub alpha_to_coverage_enabled: bool,
    /// False draws nothing: the vertex function only writes buffers.
    pub rasterization_enabled: bool,
    pub input_primitive_topology: PrimitiveTopologyClass,
}

/// One color attachment of a render pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderPipelineColorAttachmentDescriptor {
    pub pixel_format: PixelFormat,
    /// No blending writes the fragment's value.
    pub blending: Option<Blending>,
    pub write_mask: ColorWriteMask,
}

/// A color attachment's blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Blending {
    pub rgb_blend_operation: BlendOperation,
    pub alpha_blend_operation: BlendOperation,
    pub source_rgb_blend_factor: BlendFactor,
    pub destination_rgb_blend_factor: BlendFactor,
    pub source_alpha_blend_factor: BlendFactor,
    pub destination_alpha_blend_factor: BlendFactor,
}

/// How a blend joins the two products.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

/// A factor a blend multiplies by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    Zero,
    One,
    SourceColor,
    OneMinusSourceColor,
    SourceAlpha,
    OneMinusSourceAlpha,
    DestinationColor,
    OneMinusDestinationColor,
    DestinationAlpha,
    OneMinusDestinationAlpha,
    SourceAlphaSaturated,
    BlendColor,
    OneMinusBlendColor,
    BlendAlpha,
    OneMinusBlendAlpha,
    Source1Color,
    OneMinusSource1Color,
    Source1Alpha,
    OneMinusSource1Alpha,
}

/// The channels a color attachment keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColorWriteMask {
    pub red: bool,
    pub green: bool,
    pub blue: bool,
    pub alpha: bool,
}

/// The most color attachments a pass or a render pipeline has.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
pub const MAX_COLOR_ATTACHMENTS: usize = 8;

/// The kind of primitive a render pipeline draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveTopologyClass {
    Point,
    Line,
    Triangle,
}

/// Why a render pipeline was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeRenderPipelineStateError {
    /// A function of the wrong stage, or attachments the functions do not fit.
    Invalid(&'static str),
    Refused(String),
}

/// Why a compute pipeline was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeComputePipelineStateError {
    /// The function is not a compute function.
    NotCompute,
    /// A workgroup larger than the pipeline runs, or workgroup variables larger than the device holds.
    TooLarge(&'static str),
    Refused(String),
}

/// Why a command buffer was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeCommandBufferError {
    Refused,
}

/// A render pass to encode; with no attachment it draws into an area of the given size and sample count.
#[derive(Debug)]
pub struct RenderPassDescriptor<'a, M: Metal + ?Sized> {
    pub color_attachments: [Option<RenderPassColorAttachment<'a, M>>; MAX_COLOR_ATTACHMENTS],
    pub depth_attachment: Option<RenderPassDepthStencilAttachment<'a, M>>,
    pub stencil_attachment: Option<RenderPassDepthStencilAttachment<'a, M>>,
    /// The buffer occlusion counts are added into, eight bytes each, so a count spread over passes adds up.
    pub visibility_result_buffer: Option<&'a M::Buffer>,
    pub render_target_width: u32,
    pub render_target_height: u32,
    pub default_raster_sample_count: u32,
}

/// One color attachment of a pass.
#[derive(Debug)]
pub struct RenderPassColorAttachment<'a, M: Metal + ?Sized> {
    pub texture: &'a M::Texture,
    pub level: u32,
    pub slice: u32,
    pub depth_plane: u32,
    pub load_action: LoadAction,
    pub store_action: StoreAction,
    /// The color `LoadAction::Clear` writes, converted to the attachment's format.
    pub clear_color: [f64; CHANNELS],
    pub resolve: Option<ResolveTarget<'a, M>>,
}

/// What an attachment holds when a pass begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadAction {
    DontCare,
    Load,
    Clear,
}

/// What an attachment keeps when a pass ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StoreAction {
    DontCare,
    Store,
}

/// The channels of a clear color and a blend color: red, green, blue and alpha.
///
/// https://developer.apple.com/documentation/metal/mtlclearcolor
pub const CHANNELS: usize = 4;

/// The texture a multisampled attachment is resolved into when its pass ends.
#[derive(Debug)]
pub struct ResolveTarget<'a, M: Metal + ?Sized> {
    pub texture: &'a M::Texture,
    pub level: u32,
    pub slice: u32,
    pub depth_plane: u32,
}

/// The depth or the stencil attachment of a pass.
#[derive(Debug)]
pub struct RenderPassDepthStencilAttachment<'a, M: Metal + ?Sized> {
    pub texture: &'a M::Texture,
    pub level: u32,
    pub slice: u32,
    pub load_action: LoadAction,
    pub store_action: StoreAction,
    /// The depth or the stencil value `LoadAction::Clear` writes.
    pub clear_value: f64,
}

/// One command of a render pass, which later draws in the pass see until another command replaces it.
#[derive(Clone, Debug)]
pub enum RenderCommand<M: Metal + ?Sized> {
    SetRenderPipelineState(M::RenderPipelineState),
    SetDepthStencilState(M::DepthStencilState),
    /// Binds `length` bytes from `offset`, the bytes the vertex function's bounds checks see at the slot.
    SetVertexBuffer {
        buffer: M::Buffer,
        offset: u64,
        length: u64,
        index: u32,
    },
    /// Binds `length` bytes from `offset`, the bytes the fragment function's bounds checks see at the slot.
    SetFragmentBuffer {
        buffer: M::Buffer,
        offset: u64,
        length: u64,
        index: u32,
    },
    SetVertexBytes {
        bytes: Vec<u8>,
        index: u32,
    },
    SetFragmentBytes {
        bytes: Vec<u8>,
        index: u32,
    },
    SetVertexTexture {
        texture: M::Texture,
        index: u32,
    },
    SetFragmentTexture {
        texture: M::Texture,
        index: u32,
    },
    SetVertexSamplerState {
        sampler: M::SamplerState,
        index: u32,
    },
    SetFragmentSamplerState {
        sampler: M::SamplerState,
        index: u32,
    },
    SetViewports(Vec<Viewport>),
    SetScissorRects(Vec<ScissorRect>),
    SetCullMode(CullMode),
    SetFrontFacingWinding(Winding),
    SetTriangleFillMode(TriangleFillMode),
    SetDepthClipMode(DepthClipMode),
    SetDepthBias {
        depth_bias: f32,
        slope_scale: f32,
        clamp: f32,
    },
    SetBlendColor([f32; CHANNELS]),
    SetStencilReferenceValues {
        front: u32,
        back: u32,
    },
    /// Discards each fragment whose stored depth is below `min_bound` or above `max_bound`, from then on in the pass.
    ///
    /// Zero and one test nothing. Both within zero to one, the low at most the high, in a pass with depth, where
    /// [`Metal::supports_depth_test_bounds`].
    ///
    /// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/setdepthtestbounds(_:)
    SetDepthTestBounds {
        min_bound: f32,
        max_bound: f32,
    },
    /// Counts into the pass's visibility result buffer at `offset`, a multiple of eight counted into once a pass.
    SetVisibilityResultMode {
        mode: VisibilityResultMode,
        offset: u64,
    },
    DrawPrimitives {
        primitive_type: PrimitiveType,
        vertex_start: u32,
        vertex_count: u32,
        instance_count: u32,
        base_instance: u32,
    },
    DrawIndexedPrimitives {
        primitive_type: PrimitiveType,
        index_count: u32,
        index_type: IndexType,
        index_buffer: M::Buffer,
        index_buffer_offset: u64,
        instance_count: u32,
        base_vertex: i32,
        base_instance: u32,
    },
    /// Draws with the `DRAW_ARGUMENTS_BYTES` the GPU reads at `indirect_buffer_offset` when the draw runs.
    ///
    /// They are the vertex count, instance count, first vertex and first instance; a draw whose vertices or instances
    /// pass the largest index draws nothing.
    DrawPrimitivesIndirect {
        primitive_type: PrimitiveType,
        indirect_buffer: M::Buffer,
        indirect_buffer_offset: u64,
    },
    /// Draws with the `DRAW_INDEXED_ARGUMENTS_BYTES` the GPU reads at `indirect_buffer_offset` when the draw runs.
    ///
    /// They are the index count, instance count, first index, base vertex and first instance; a draw whose indices
    /// reach past the index buffer's end, or whose instances pass the largest index, draws nothing.
    DrawIndexedPrimitivesIndirect {
        primitive_type: PrimitiveType,
        index_type: IndexType,
        index_buffer: M::Buffer,
        index_buffer_offset: u64,
        indirect_buffer: M::Buffer,
        indirect_buffer_offset: u64,
    },
}

/// The bytes of an indirect draw's arguments: four `u32`s.
///
/// https://developer.apple.com/documentation/metal/mtldrawprimitivesindirectarguments
pub const DRAW_ARGUMENTS_BYTES: u64 = 16;

/// The bytes of an indirect indexed draw's arguments: five 32-bit integers.
///
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
pub const DRAW_INDEXED_ARGUMENTS_BYTES: u64 = 20;

/// The bytes of an indirect dispatch's arguments: three `u32` threadgroup counts.
///
/// https://developer.apple.com/documentation/metal/mtldispatchthreadgroupsindirectarguments
pub const DISPATCH_ARGUMENTS_BYTES: u64 = 12;

/// The arguments of an indirect draw, as the GPU reads them from `DRAW_ARGUMENTS_BYTES`.
///
/// https://developer.apple.com/documentation/metal/mtldrawprimitivesindirectarguments
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DrawPrimitivesIndirectArguments {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub vertex_start: u32,
    pub base_instance: u32,
}

/// The arguments of an indirect indexed draw, as the GPU reads them from `DRAW_INDEXED_ARGUMENTS_BYTES`.
///
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DrawIndexedPrimitivesIndirectArguments {
    pub index_count: u32,
    pub instance_count: u32,
    pub index_start: u32,
    pub base_vertex: i32,
    pub base_instance: u32,
}

/// The arguments of an indirect dispatch, as the GPU reads them from `DISPATCH_ARGUMENTS_BYTES`.
///
/// https://developer.apple.com/documentation/metal/mtldispatchthreadgroupsindirectarguments
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DispatchThreadgroupsIndirectArguments {
    /// The threadgroups on each axis: width, height and depth.
    pub threadgroups_per_grid: Size,
}

/// What the offset of indirect arguments in their buffer is a multiple of.
///
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/drawprimitives(type:indirectbuffer:indirectbufferoffset:)
pub const INDIRECT_ARGUMENTS_ALIGNMENT: u64 = 4;

/// A viewport, in pixels and depth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub znear: f64,
    pub zfar: f64,
}

/// A scissor rectangle in pixels, inside the pass's area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScissorRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Which faces a draw drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    Back,
}

/// The winding of a triangle that faces the viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Winding {
    Clockwise,
    CounterClockwise,
}

/// How triangles are filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TriangleFillMode {
    Fill,
    Lines,
}

/// Whether depth outside the viewport's range is clipped or clamped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DepthClipMode {
    Clip,
    Clamp,
}

/// How occlusion counts are kept while drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VisibilityResultMode {
    Disabled,
    /// Non-zero where any sample passed.
    Boolean,
    /// The number of samples that passed.
    Counting,
}

/// The primitives a draw makes of its vertices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Point,
    Line,
    LineStrip,
    Triangle,
    TriangleStrip,
}

/// The width of index values; the largest value restarts strips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexType {
    UInt16,
    UInt32,
}

/// Why a render pass was not encoded; nothing of it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodeRenderCommandsError {
    /// The command buffer was already committed.
    Committed,
    /// An attachment that does not fit the pass, or a command that breaks a rule of the pass.
    Invalid(&'static str),
    /// A range outside its resource.
    OutOfRange(&'static str),
    /// A draw whose functions read a slot no command filled, or filled with the wrong kind of resource.
    Unbound(&'static str),
}

/// One copy or fill.
#[derive(Clone, Debug)]
pub enum BlitCommand<M: Metal + ?Sized> {
    /// Copies bytes; one buffer may be both when the ranges do not overlap.
    CopyFromBufferToBuffer {
        source_buffer: M::Buffer,
        source_offset: u64,
        destination_buffer: M::Buffer,
        destination_offset: u64,
        size: u64,
    },
    CopyFromBufferToTexture {
        source_buffer: M::Buffer,
        source_offset: u64,
        source_bytes_per_row: u64,
        source_bytes_per_image: u64,
        source_size: Size,
        destination_texture: M::Texture,
        destination_slice: u32,
        destination_level: u32,
        destination_origin: Origin,
        options: BlitOption,
    },
    CopyFromTextureToBuffer {
        source_texture: M::Texture,
        source_slice: u32,
        source_level: u32,
        source_origin: Origin,
        source_size: Size,
        destination_buffer: M::Buffer,
        destination_offset: u64,
        destination_bytes_per_row: u64,
        destination_bytes_per_image: u64,
        options: BlitOption,
    },
    /// Copies texels of one slice and level; one texture may be both when the regions do not overlap.
    CopyFromTextureToTexture {
        source_texture: M::Texture,
        source_slice: u32,
        source_level: u32,
        source_origin: Origin,
        source_size: Size,
        destination_texture: M::Texture,
        destination_slice: u32,
        destination_level: u32,
        destination_origin: Origin,
    },
    FillBuffer {
        buffer: M::Buffer,
        offset: u64,
        length: u64,
        value: u8,
    },
    /// Fills each level but the first of every slice with the level above scaled down, filtered as the GPU filters.
    GenerateMipmaps { texture: M::Texture },
}

/// An extent in texels, or a count of threadgroups.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

/// A point in a texture level, in texels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Origin {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

/// Which aspect of a depth and stencil texture a copy between a buffer and the texture moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlitOption {
    None,
    DepthFromDepthStencil,
    StencilFromDepthStencil,
}

/// Why copies and fills were not encoded; none of them is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodeBlitCommandsError {
    /// The command buffer was already committed.
    Committed,
    /// A copy between formats or aspects Metal does not copy, or between overlapping ranges.
    Invalid(&'static str),
    /// A range outside its resource.
    OutOfRange(&'static str),
}

/// One command of a compute pass, which later dispatches see until another command replaces it.
#[derive(Clone, Debug)]
pub enum ComputeCommand<M: Metal + ?Sized> {
    SetComputePipelineState(M::ComputePipelineState),
    /// Binds `length` bytes from `offset`, the bytes the function's bounds checks see at the slot.
    SetBuffer {
        buffer: M::Buffer,
        offset: u64,
        length: u64,
        index: u32,
    },
    SetBytes {
        bytes: Vec<u8>,
        index: u32,
    },
    SetTexture {
        texture: M::Texture,
        index: u32,
    },
    SetSamplerState {
        sampler: M::SamplerState,
        index: u32,
    },
    DispatchThreadgroups(Size),
    /// Dispatches the threadgroups the GPU reads as `DISPATCH_ARGUMENTS_BYTES` at `indirect_buffer_offset` once run.
    ///
    /// One whose count on an axis passes `max_threadgroups`, or whose threads on an axis pass the largest `u32`,
    /// dispatches nothing.
    DispatchThreadgroupsIndirect {
        indirect_buffer: M::Buffer,
        indirect_buffer_offset: u64,
        max_threadgroups: u32,
    },
}

/// Why dispatches were not encoded; none of them is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodeComputeCommandsError {
    /// The command buffer was already committed.
    Committed,
    Invalid(&'static str),
    OutOfRange(&'static str),
    /// A dispatch whose function reads a slot no command filled, or filled with the wrong kind of resource.
    Unbound(&'static str),
}

/// Why a command buffer was not committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitError {
    AlreadyCommitted,
}

/// Why a command buffer was not waited for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaitUntilCompletedError {
    /// Waiting for a command buffer not committed would never end.
    NotCommitted,
}

/// How far the GPU has come with a command buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandBufferStatus {
    NotEnqueued,
    Enqueued,
    Committed,
    Scheduled,
    Completed,
    Error,
}

/// Why a layer was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MakeLayerError {
    /// A window whose handle is not an AppKit view's.
    NotAppKit,
    /// A call off the main thread, where AppKit's views are not reached.
    NotMainThread,
}

/// How a layer's drawables are made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerDescriptor {
    /// One of `Bgra8Unorm`, `Bgra8UnormSrgb`, `Rgba16Float` and `Rgb10a2Unorm`.
    pub pixel_format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub display_sync_enabled: bool,
}

/// Why a layer was not configured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigureLayerError {
    /// A format no layer takes, or an empty size.
    Invalid(&'static str),
}

/// Why no drawable was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NextDrawableError {
    /// The layer was never configured.
    NotConfigured,
    /// None was free before Metal's wait ended.
    TimedOut,
}

/// Why a drawable was not presented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentDrawableError {
    /// The command buffer was already committed.
    Committed,
    /// The drawable was presented before.
    AlreadyPresented,
}

/// A Metal device made on its own, on the system's default GPU.
pub trait StandaloneMetal: Metal {
    type Config;

    fn new(config: Self::Config) -> Self;
}

/// The largest anisotropy a sampler takes.
///
/// https://developer.apple.com/documentation/metal/mtlsamplerdescriptor/maxanisotropy
pub const MAX_ANISOTROPY: u32 = 16;

/// The most buffer, texture and sampler slots one stage reads, each kind counted apart, buffers first.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
pub const MAX_BUFFER_SLOTS: u32 = 31;

/// The most sampler slots one stage reads.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
pub const MAX_SAMPLER_SLOTS: u32 = 16;

/// The longest side of a 2D, cube or multisampled texture on the GPUs that make the longest, those of the Apple10 family.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
pub const MAX_TEXTURE_SIDE: u32 = 32_768;

/// The most texture slots one stage reads.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
pub const MAX_TEXTURE_SLOTS: u32 = 128;

/// The most bytes between the elements of one vertex buffer, WebGPU's most and so the interface's.
///
/// https://www.w3.org/TR/webgpu/#dom-supported-limits-maxvertexbufferarraystride
pub const MAX_VERTEX_STRIDE: u32 = 2048;

/// The most viewports and scissor rectangles one draw has.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
pub const MAX_VIEWPORTS: usize = 16;

/// The bytes of one occlusion count in a visibility result buffer.
///
/// https://developer.apple.com/documentation/metal/mtlvisibilityresultmode/counting
pub const VISIBILITY_RESULT_BYTES: u64 = 8;
