//! Metal's own calls through objc2-metal, each checked against what it touches just before it is made.
//!
//! Functions are written by naga with every access bounds-checked and vertex inputs read by the function itself, so a
//! draw is sound once every slot its functions read holds a resource of the right kind and size: each encode checks
//! that, and passes the functions the lengths their bounds checks read.
#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::fmt;
use std::ptr::NonNull;
use std::rc::Rc;

use objc2::MainThreadMarker;
use objc2::rc::{Retained, autoreleasepool};
use objc2_core_foundation::CGSize;
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use objc2::runtime::ProtocolObject;
use objc2_foundation::{NSRange, NSString, NSUInteger};
use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLBlitCommandEncoder, MTLBlitOption, MTLBuffer,
    MTLClearColor, MTLColorWriteMask, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder,
    MTLCommandQueue, MTLCompareFunction, MTLCompileOptions, MTLComputeCommandEncoder,
    MTLComputePipelineState, MTLCreateSystemDefaultDevice, MTLCullMode, MTLDepthClipMode,
    MTLDepthStencilDescriptor, MTLDepthStencilState, MTLDevice, MTLFunction, MTLIndexType,
    MTLLanguageVersion, MTLLibrary, MTLLoadAction, MTLOrigin, MTLPixelFormat,
    MTLPrimitiveTopologyClass, MTLPrimitiveType, MTLRenderCommandEncoder, MTLRenderPassDescriptor,
    MTLRenderPipelineDescriptor, MTLRenderPipelineState, MTLResourceOptions, MTLSamplerAddressMode,
    MTLSamplerBorderColor, MTLSamplerDescriptor, MTLSamplerMinMagFilter, MTLSamplerMipFilter,
    MTLSamplerState, MTLScissorRect, MTLSize, MTLStencilDescriptor, MTLStencilOperation,
    MTLStorageMode, MTLStoreAction, MTLTexture, MTLTextureDescriptor, MTLTextureType,
    MTLTextureUsage, MTLTriangleFillMode, MTLViewport, MTLVisibilityResultMode, MTLWinding,
};
use wcontainers_metal::{
    ConfigureLayerError, LayerDescriptor, MakeLayerError, NextDrawableError, PresentDrawableError,
    BlendFactor, BlendOperation, BlitCommand, BlitOption, CommandBufferStatus, CommitError,
    CompareFunction, ComputeCommand, CullMode, DepthClipMode, DepthStencilDescriptor,
    EncodeBlitCommandsError, EncodeComputeCommandsError, EncodeRenderCommandsError, FunctionSource,
    IndexType, LoadAction, MAX_BUFFER_SLOTS, MAX_COLOR_ATTACHMENTS, MAX_SAMPLER_SLOTS,
    MAX_TEXTURE_SLOTS, MAX_VIEWPORTS, MakeBufferError, MakeCommandBufferError,
    MakeComputePipelineStateError, MakeDepthStencilStateError, MakeFunctionError,
    MakeRenderPipelineStateError, MakeSamplerStateError, MakeTextureError, MakeTextureViewError,
    Metal, Origin, PixelFormat, PrimitiveTopologyClass, PrimitiveType, ReadContentsError,
    RenderCommand, RenderPassDescriptor, RenderPipelineDescriptor, SamplerAddressMode,
    SamplerBorderColor, SamplerDescriptor, SamplerMinMagFilter, SamplerMipFilter, Size,
    StandaloneMetal, StencilDescriptor, StencilOperation, StoreAction, TextureDescriptor,
    TextureType, TextureViewDescriptor, TriangleFillMode, VertexFormat, VertexStepFunction,
    VisibilityResultMode, WaitUntilCompletedError, Winding, WriteContentsError,
};

/// The Metal Shading Language version functions are written in and compiled as.
///
/// https://developer.apple.com/documentation/metal/mtllanguageversion/version3_1
const LANGUAGE_VERSION: (u8, u8) = (3, 1);

/// The bytes of one occlusion result in a visibility result buffer.
///
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/setvisibilityresultmode(_:offset:)
const VISIBILITY_RESULT_BYTES: u64 = 8;

/// The bytes of the buffer bound to vertex buffer slots no command filled, whose length the function reads as zero.
const EMPTY_BUFFER_BYTES: usize = 16;

/// The largest anisotropy a sampler takes.
///
/// https://developer.apple.com/documentation/metal/mtlsamplerdescriptor/maxanisotropy
const MAX_ANISOTROPY: u32 = 16;

/// The system's default GPU and one command queue on it.
pub struct DirectMetal {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    /// Bound to vertex buffer slots no command filled, with length zero passed to the function.
    empty: Retained<ProtocolObject<dyn MTLBuffer>>,
}

/// A buffer of memory the CPU and the GPU share, and the command buffers that use it.
#[derive(Clone)]
pub struct DirectBuffer(Rc<BufferState>);

struct BufferState {
    raw: Retained<ProtocolObject<dyn MTLBuffer>>,
    length: u64,
    users: RefCell<Vec<DirectCommandBuffer>>,
}

/// A texture or a view of one, with what it was made as.
#[derive(Clone)]
pub struct DirectTexture(Rc<TextureState>);

struct TextureState {
    raw: Retained<ProtocolObject<dyn MTLTexture>>,
    texture_type: TextureType,
    pixel_format: PixelFormat,
    width: u32,
    height: u32,
    depth: u32,
    levels: u32,
    /// Layers, cube faces counted one each.
    slices: u32,
    sample_count: u32,
    render_target: bool,
    pixel_format_view: bool,
    /// The texture's storage, the same for every view of it, to tell overlapping copies.
    storage: usize,
}

/// A sampler state.
#[derive(Clone)]
pub struct DirectSamplerState(Retained<ProtocolObject<dyn MTLSamplerState>>);

/// A depth and stencil state.
#[derive(Clone)]
pub struct DirectDepthStencilState(Retained<ProtocolObject<dyn MTLDepthStencilState>>);

/// A compiled entry point and the slots it reads.
#[derive(Clone)]
pub struct DirectFunction(Rc<FunctionState>);

struct FunctionState {
    raw: Retained<ProtocolObject<dyn MTLFunction>>,
    stage: naga::ShaderStage,
    reads: Reads,
}

/// What a function reads, which a draw or dispatch must have bound.
#[derive(Clone, Default)]
struct Reads {
    /// Buffer slots and the least bytes each must hold past its offset.
    buffers: Vec<(u32, u64)>,
    textures: Vec<(u32, TextureRead)>,
    samplers: Vec<u32>,
    /// The vertex buffer slots it reads its inputs from.
    vertex_buffers: Vec<u32>,
    /// What each length in the buffer of lengths is the length of, in naga's order.
    sizes: Vec<SizeOf>,
    sizes_index: u32,
    /// The immediates slot and the bytes it must hold.
    immediates: Option<(u32, u64)>,
    workgroup_size: [u32; 3],
    /// The bytes of each workgroup variable a compute function reads, in naga's order of its threadgroup arguments.
    threadgroup_lengths: Vec<u64>,
}

/// The texture a function reads at one slot.
#[derive(Clone, Copy, PartialEq, Eq)]
struct TextureRead {
    texture_type: TextureType,
    depth: bool,
}

/// One length the bounds checks read.
#[derive(Clone, Copy)]
enum SizeOf {
    /// The bytes past the offset of the buffer at a slot.
    Slot(u32),
    /// A global the function does not use, whose length is never read.
    Unused,
}

/// A render pipeline and what it was made for.
#[derive(Clone)]
pub struct DirectRenderPipelineState(Rc<RenderPipelineState>);

struct RenderPipelineState {
    raw: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    vertex: Rc<FunctionState>,
    fragment: Option<Rc<FunctionState>>,
    colors: [Option<PixelFormat>; MAX_COLOR_ATTACHMENTS],
    depth: Option<PixelFormat>,
    stencil: Option<PixelFormat>,
    sample_count: u32,
    topology: PrimitiveTopologyClass,
}

/// A compute pipeline and its function.
#[derive(Clone)]
pub struct DirectComputePipelineState(Rc<ComputePipelineState>);

struct ComputePipelineState {
    raw: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    function: Rc<FunctionState>,
}

/// A command buffer and whether it was committed.
#[derive(Clone)]
pub struct DirectCommandBuffer(Rc<CommandBufferState>);

struct CommandBufferState {
    raw: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    committed: Cell<bool>,
}

/// A window view's Metal layer, and how its drawables were last configured.
#[derive(Clone)]
pub struct DirectLayer(Rc<LayerState>);

struct LayerState {
    raw: Retained<CAMetalLayer>,
    configured: Cell<Option<LayerDescriptor>>,
}

/// A drawable of a layer, its texture, and whether it was presented.
#[derive(Clone)]
pub struct DirectDrawable(Rc<DrawableState>);

struct DrawableState {
    raw: Retained<ProtocolObject<dyn CAMetalDrawable>>,
    texture: DirectTexture,
    presented: Cell<bool>,
}

macro_rules! opaque_debug {
    ($($ty:ident),*) => {
        $(impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(stringify!($ty))
            }
        })*
    };
}

opaque_debug!(
    DirectBuffer,
    DirectTexture,
    DirectSamplerState,
    DirectDepthStencilState,
    DirectFunction,
    DirectRenderPipelineState,
    DirectComputePipelineState,
    DirectCommandBuffer,
    DirectLayer,
    DirectDrawable
);

impl StandaloneMetal for DirectMetal {
    type Config = ();

    fn new(_config: ()) -> DirectMetal {
        let device = MTLCreateSystemDefaultDevice().expect("a Metal device");
        let queue = device.newCommandQueue().expect("a command queue");
        let empty = device
            .newBufferWithLength_options(EMPTY_BUFFER_BYTES, MTLResourceOptions::StorageModeShared)
            .expect("a buffer");
        DirectMetal {
            device,
            queue,
            empty,
        }
    }
}

impl DirectBuffer {
    /// Waits for the committed command buffers that use the buffer; refuses when one is not committed.
    fn settle(&self) -> Result<(), ()> {
        let mut users = self.0.users.borrow_mut();
        if users.iter().any(|user| !user.0.committed.get()) {
            return Err(());
        }
        for user in users.drain(..) {
            user.0.raw.waitUntilCompleted();
        }
        Ok(())
    }

    /// Notes that a command buffer uses the buffer.
    fn used_by(&self, command_buffer: &DirectCommandBuffer) {
        let mut users = self.0.users.borrow_mut();
        users.retain(|user| user.0.raw.status() != MTLCommandBufferStatus::Completed);
        if !users
            .iter()
            .any(|user| Rc::ptr_eq(&user.0, &command_buffer.0))
        {
            users.push(command_buffer.clone());
        }
    }

    /// Returns whether `length` bytes at `offset` lie inside the buffer.
    fn holds(&self, offset: u64, length: u64) -> bool {
        offset
            .checked_add(length)
            .is_some_and(|end| end <= self.0.length)
    }
}

impl Metal for DirectMetal {
    type Buffer = DirectBuffer;
    type Texture = DirectTexture;
    type SamplerState = DirectSamplerState;
    type DepthStencilState = DirectDepthStencilState;
    type Function = DirectFunction;
    type RenderPipelineState = DirectRenderPipelineState;
    type ComputePipelineState = DirectComputePipelineState;
    type CommandBuffer = DirectCommandBuffer;
    type Layer = DirectLayer;
    type Drawable = DirectDrawable;

    fn name(&self) -> String {
        self.device.name().to_string()
    }

    fn make_buffer(&mut self, length: u64) -> Result<DirectBuffer, MakeBufferError> {
        if length > self.device.maxBufferLength() as u64 {
            return Err(MakeBufferError::TooLong);
        }
        // Metal makes no buffer of no bytes, so such a buffer holds one unread byte.
        let raw = self
            .device
            .newBufferWithLength_options(
                (length as NSUInteger).max(1),
                MTLResourceOptions::StorageModeShared,
            )
            .ok_or(MakeBufferError::Refused)?;
        Ok(DirectBuffer(Rc::new(BufferState {
            raw,
            length,
            users: RefCell::new(Vec::new()),
        })))
    }

    fn write_contents(
        &mut self,
        buffer: &DirectBuffer,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), WriteContentsError> {
        if !buffer.holds(offset, bytes.len() as u64) {
            return Err(WriteContentsError::OutOfRange);
        }
        buffer
            .settle()
            .map_err(|()| WriteContentsError::UsedByUncommittedWork)?;
        let contents = buffer.0.raw.contents().cast::<u8>();
        #[expect(unsafe_code, reason = "the copy into a buffer's shared memory")]
        // SAFETY: the range lies inside the buffer, which is at least as long; no GPU work uses it, since every command
        // buffer that did has completed and none other was encoded since; the CPU touches it only in these copies.
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                contents.as_ptr().add(offset as usize),
                bytes.len(),
            )
        };
        Ok(())
    }

    fn read_contents(
        &mut self,
        buffer: &DirectBuffer,
        offset: u64,
        bytes: &mut [u8],
    ) -> Result<(), ReadContentsError> {
        if !buffer.holds(offset, bytes.len() as u64) {
            return Err(ReadContentsError::OutOfRange);
        }
        buffer
            .settle()
            .map_err(|()| ReadContentsError::UsedByUncommittedWork)?;
        let contents = buffer.0.raw.contents().cast::<u8>();
        #[expect(unsafe_code, reason = "the copy out of a buffer's shared memory")]
        // SAFETY: as for `write_contents`: inside the buffer, and no GPU work uses it.
        unsafe {
            std::ptr::copy_nonoverlapping(
                contents.as_ptr().add(offset as usize),
                bytes.as_mut_ptr(),
                bytes.len(),
            )
        };
        Ok(())
    }

    fn make_texture(
        &mut self,
        descriptor: &TextureDescriptor,
    ) -> Result<DirectTexture, MakeTextureError> {
        let d = descriptor;
        let facts = format_facts(d.pixel_format);
        let one_slice = matches!(
            d.texture_type,
            TextureType::Type2D | TextureType::Type2DMultisample | TextureType::Type3D
        );
        let checks = [
            (
                d.width == 0 || d.height == 0 || d.depth == 0,
                "an empty size",
            ),
            (
                d.texture_type != TextureType::Type3D && d.depth != 1,
                "a depth other than one outside 3D",
            ),
            (
                one_slice && d.array_length != 1 || d.array_length == 0,
                "an array length the type does not take",
            ),
            (
                matches!(
                    d.texture_type,
                    TextureType::TypeCube | TextureType::TypeCubeArray
                ) && d.width != d.height,
                "a cube that is not square",
            ),
            (
                d.mipmap_level_count == 0
                    || d.mipmap_level_count > full_chain(d.width.max(d.height).max(d.depth)),
                "more levels than the size has",
            ),
            (
                (d.sample_count > 1) != (d.texture_type == TextureType::Type2DMultisample)
                    || ![1, 2, 4, 8].contains(&d.sample_count)
                    || d.sample_count > 1 && d.mipmap_level_count != 1,
                "a sample count the type does not take",
            ),
            (
                facts.block != (1, 1)
                    && (d.texture_type == TextureType::Type3D || d.sample_count > 1),
                "a compressed format the type does not take",
            ),
            (
                facts.depth_or_stencil && d.texture_type == TextureType::Type3D,
                "a 3D depth or stencil texture",
            ),
            (
                !self
                    .device
                    .supportsTextureSampleCount(d.sample_count as NSUInteger),
                "a sample count the device lacks",
            ),
        ];
        if let Some((_, why)) = checks.iter().find(|(failed, _)| *failed) {
            return Err(MakeTextureError::Invalid(why));
        }
        let raw_descriptor = MTLTextureDescriptor::new();
        raw_descriptor.setTextureType(texture_type(d.texture_type));
        raw_descriptor.setPixelFormat(pixel_format(d.pixel_format));
        raw_descriptor.setStorageMode(MTLStorageMode::Private);
        let mut usage = MTLTextureUsage::empty();
        for (wanted, flag) in [
            (d.usage.shader_read, MTLTextureUsage::ShaderRead),
            (d.usage.shader_write, MTLTextureUsage::ShaderWrite),
            (d.usage.render_target, MTLTextureUsage::RenderTarget),
            (d.usage.pixel_format_view, MTLTextureUsage::PixelFormatView),
        ] {
            if wanted {
                usage |= flag;
            }
        }
        raw_descriptor.setUsage(usage);
        #[expect(unsafe_code, reason = "the sizes of a texture to make")]
        // SAFETY: every size, count and length was checked above against what the type and format take.
        unsafe {
            raw_descriptor.setWidth(d.width as NSUInteger);
            raw_descriptor.setHeight(d.height as NSUInteger);
            raw_descriptor.setDepth(d.depth as NSUInteger);
            raw_descriptor.setMipmapLevelCount(d.mipmap_level_count as NSUInteger);
            raw_descriptor.setSampleCount(d.sample_count as NSUInteger);
            raw_descriptor.setArrayLength(d.array_length as NSUInteger);
        }
        let raw = self
            .device
            .newTextureWithDescriptor(&raw_descriptor)
            .ok_or(MakeTextureError::Refused)?;
        let faces = match d.texture_type {
            TextureType::TypeCube | TextureType::TypeCubeArray => CUBE_FACES,
            _ => 1,
        };
        let storage = Retained::as_ptr(&raw).cast::<c_void>() as usize;
        Ok(DirectTexture(Rc::new(TextureState {
            raw,
            texture_type: d.texture_type,
            pixel_format: d.pixel_format,
            width: d.width,
            height: d.height,
            depth: d.depth,
            levels: d.mipmap_level_count,
            slices: d.array_length * faces,
            sample_count: d.sample_count,
            render_target: d.usage.render_target,
            pixel_format_view: d.usage.pixel_format_view,
            storage,
        })))
    }

    fn make_texture_view(
        &mut self,
        texture: &DirectTexture,
        descriptor: &TextureViewDescriptor,
    ) -> Result<DirectTexture, MakeTextureViewError> {
        let t = &texture.0;
        let d = descriptor;
        let levels = d.first_level.checked_add(d.level_count);
        let slices = d.first_slice.checked_add(d.slice_count);
        if d.level_count == 0
            || d.slice_count == 0
            || levels.is_none_or(|end| end > t.levels)
            || slices.is_none_or(|end| end > t.slices)
        {
            return Err(MakeTextureViewError::OutOfRange);
        }
        let from = format_facts(t.pixel_format);
        let to = format_facts(d.pixel_format);
        let same_format =
            d.pixel_format == t.pixel_format || from.srgb_twin == Some(d.pixel_format);
        let reinterpreted = t.pixel_format_view
            && !from.depth_or_stencil
            && !to.depth_or_stencil
            && from.block == (1, 1)
            && to.block == (1, 1)
            && from.bytes == to.bytes;
        let stencil_of = t.pixel_format == PixelFormat::Depth32FloatStencil8
            && d.pixel_format == PixelFormat::X32Stencil8;
        let slices_fit = match d.texture_type {
            TextureType::Type2D | TextureType::Type2DMultisample | TextureType::Type3D => {
                d.slice_count == 1
            }
            TextureType::TypeCube => d.slice_count == CUBE_FACES,
            TextureType::TypeCubeArray => d.slice_count.is_multiple_of(CUBE_FACES),
            TextureType::Type2DArray => true,
        };
        let type_fits = match t.texture_type {
            TextureType::Type3D => d.texture_type == TextureType::Type3D,
            TextureType::Type2DMultisample => d.texture_type == TextureType::Type2DMultisample,
            TextureType::Type2D | TextureType::Type2DArray => {
                matches!(
                    d.texture_type,
                    TextureType::Type2D | TextureType::Type2DArray
                )
            }
            TextureType::TypeCube | TextureType::TypeCubeArray => matches!(
                d.texture_type,
                TextureType::Type2D
                    | TextureType::Type2DArray
                    | TextureType::TypeCube
                    | TextureType::TypeCubeArray
            ),
        };
        let square = t.width == t.height
            || !matches!(
                d.texture_type,
                TextureType::TypeCube | TextureType::TypeCubeArray
            );
        if !(same_format || reinterpreted || stencil_of) || !slices_fit || !type_fits || !square {
            return Err(MakeTextureViewError::Incompatible);
        }
        #[expect(unsafe_code, reason = "the call that makes a texture view")]
        // SAFETY: the levels and slices lie inside the texture, the format is the texture's, its sRGB twin, the stencil
        // of its depth and stencil, or one of the same size where the texture was made for such views, and the type is
        // one Metal makes of the texture's for that many slices.
        let raw = unsafe {
            t.raw
                .newTextureViewWithPixelFormat_textureType_levels_slices(
                    pixel_format(d.pixel_format),
                    texture_type(d.texture_type),
                    NSRange::new(d.first_level as usize, d.level_count as usize),
                    NSRange::new(d.first_slice as usize, d.slice_count as usize),
                )
        }
        .ok_or(MakeTextureViewError::Refused)?;
        let size = |n: u32| (n >> d.first_level).max(1);
        Ok(DirectTexture(Rc::new(TextureState {
            raw,
            texture_type: d.texture_type,
            pixel_format: d.pixel_format,
            width: size(t.width),
            height: size(t.height),
            depth: if t.texture_type == TextureType::Type3D {
                size(t.depth)
            } else {
                1
            },
            levels: d.level_count,
            slices: d.slice_count,
            sample_count: t.sample_count,
            render_target: t.render_target,
            pixel_format_view: false,
            storage: t.storage,
        })))
    }

    fn make_sampler_state(
        &mut self,
        descriptor: &SamplerDescriptor,
    ) -> Result<DirectSamplerState, MakeSamplerStateError> {
        let d = descriptor;
        if d.max_anisotropy == 0 || d.max_anisotropy > MAX_ANISOTROPY {
            return Err(MakeSamplerStateError::Invalid("an anisotropy of 1 to 16"));
        }
        if d.lod_min_clamp.is_nan() || d.lod_max_clamp.is_nan() || d.lod_min_clamp > d.lod_max_clamp
        {
            return Err(MakeSamplerStateError::Invalid(
                "a level range that is empty",
            ));
        }
        let raw_descriptor = MTLSamplerDescriptor::new();
        raw_descriptor.setMinFilter(min_mag_filter(d.min_filter));
        raw_descriptor.setMagFilter(min_mag_filter(d.mag_filter));
        raw_descriptor.setMipFilter(match d.mip_filter {
            SamplerMipFilter::NotMipmapped => MTLSamplerMipFilter::NotMipmapped,
            SamplerMipFilter::Nearest => MTLSamplerMipFilter::Nearest,
            SamplerMipFilter::Linear => MTLSamplerMipFilter::Linear,
        });
        raw_descriptor.setSAddressMode(address_mode(d.s_address_mode));
        raw_descriptor.setTAddressMode(address_mode(d.t_address_mode));
        raw_descriptor.setRAddressMode(address_mode(d.r_address_mode));
        raw_descriptor.setBorderColor(match d.border_color {
            SamplerBorderColor::TransparentBlack => MTLSamplerBorderColor::TransparentBlack,
            SamplerBorderColor::OpaqueBlack => MTLSamplerBorderColor::OpaqueBlack,
            SamplerBorderColor::OpaqueWhite => MTLSamplerBorderColor::OpaqueWhite,
        });
        raw_descriptor.setLodMinClamp(d.lod_min_clamp);
        raw_descriptor.setLodMaxClamp(d.lod_max_clamp);
        raw_descriptor.setMaxAnisotropy(d.max_anisotropy as NSUInteger);
        if let Some(compare) = d.compare_function {
            raw_descriptor.setCompareFunction(compare_function(compare));
        }
        self.device
            .newSamplerStateWithDescriptor(&raw_descriptor)
            .map(DirectSamplerState)
            .ok_or(MakeSamplerStateError::Refused)
    }

    fn make_depth_stencil_state(
        &mut self,
        descriptor: &DepthStencilDescriptor,
    ) -> Result<DirectDepthStencilState, MakeDepthStencilStateError> {
        let raw_descriptor = MTLDepthStencilDescriptor::new();
        raw_descriptor.setDepthCompareFunction(compare_function(descriptor.depth_compare_function));
        raw_descriptor.setDepthWriteEnabled(descriptor.depth_write_enabled);
        let stencil = |face: &StencilDescriptor| {
            let raw = MTLStencilDescriptor::new();
            raw.setStencilCompareFunction(compare_function(face.stencil_compare_function));
            raw.setStencilFailureOperation(stencil_operation(face.stencil_failure_operation));
            raw.setDepthFailureOperation(stencil_operation(face.depth_failure_operation));
            raw.setDepthStencilPassOperation(stencil_operation(face.depth_stencil_pass_operation));
            raw.setReadMask(face.read_mask);
            raw.setWriteMask(face.write_mask);
            raw
        };
        if let Some(face) = &descriptor.front_face_stencil {
            raw_descriptor.setFrontFaceStencil(Some(&stencil(face)));
        }
        if let Some(face) = &descriptor.back_face_stencil {
            raw_descriptor.setBackFaceStencil(Some(&stencil(face)));
        }
        self.device
            .newDepthStencilStateWithDescriptor(&raw_descriptor)
            .map(DirectDepthStencilState)
            .ok_or(MakeDepthStencilStateError::Refused)
    }

    fn make_function(
        &mut self,
        source: &FunctionSource<'_>,
    ) -> Result<DirectFunction, MakeFunctionError> {
        let (options, pipeline, reads) = translation(source)?;
        let (text, info) =
            naga::back::msl::write_string(source.module, source.info, &options, &pipeline)
                .map_err(|error| MakeFunctionError::Translation(error.to_string()))?;
        let name = match info.entry_point_names.first() {
            Some(Ok(name)) => name.clone(),
            Some(Err(error)) => return Err(MakeFunctionError::Translation(error.to_string())),
            None => {
                return Err(MakeFunctionError::Translation(
                    "no entry point written".into(),
                ));
            }
        };
        let compile_options = MTLCompileOptions::new();
        compile_options.setLanguageVersion(MTLLanguageVersion::Version3_1);
        let library = self
            .device
            .newLibraryWithSource_options_error(&NSString::from_str(&text), Some(&compile_options))
            .map_err(|error| {
                MakeFunctionError::Compilation(error.localizedDescription().to_string())
            })?;
        let raw = library
            .newFunctionWithName(&NSString::from_str(&name))
            .ok_or_else(|| MakeFunctionError::Compilation(format!("no function {name}")))?;
        Ok(DirectFunction(Rc::new(FunctionState {
            raw,
            stage: source.stage,
            reads,
        })))
    }

    fn make_render_pipeline_state(
        &mut self,
        descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> Result<DirectRenderPipelineState, MakeRenderPipelineStateError> {
        let d = descriptor;
        let invalid = MakeRenderPipelineStateError::Invalid;
        if d.vertex_function.0.stage != naga::ShaderStage::Vertex {
            return Err(invalid("a vertex function of another stage"));
        }
        if d.fragment_function
            .is_some_and(|f| f.0.stage != naga::ShaderStage::Fragment)
        {
            return Err(invalid("a fragment function of another stage"));
        }
        if ![1, 2, 4, 8].contains(&d.raster_sample_count) {
            return Err(invalid("a sample count of 1, 2, 4 or 8"));
        }
        if d.depth_attachment_pixel_format
            .is_some_and(|f| !format_facts(f).depth)
            || d.stencil_attachment_pixel_format
                .is_some_and(|f| !format_facts(f).stencil)
        {
            return Err(invalid("a depth or stencil format without that aspect"));
        }
        let raw_descriptor = MTLRenderPipelineDescriptor::new();
        raw_descriptor.setVertexFunction(Some(&d.vertex_function.0.raw));
        #[expect(
            unsafe_code,
            reason = "the fragment function and the counts of a pipeline"
        )]
        // SAFETY: the function is a fragment function, the sample count one Metal takes, and the topology class names
        // one of Metal's three.
        unsafe {
            raw_descriptor.setFragmentFunction(d.fragment_function.map(|f| &*f.0.raw));
            raw_descriptor.setRasterSampleCount(d.raster_sample_count as NSUInteger);
            raw_descriptor.setInputPrimitiveTopology(match d.input_primitive_topology {
                PrimitiveTopologyClass::Point => MTLPrimitiveTopologyClass::Point,
                PrimitiveTopologyClass::Line => MTLPrimitiveTopologyClass::Line,
                PrimitiveTopologyClass::Triangle => MTLPrimitiveTopologyClass::Triangle,
            });
        }
        let attachments = raw_descriptor.colorAttachments();
        for (index, attachment) in d.color_attachments.iter().enumerate() {
            let Some(attachment) = attachment else {
                continue;
            };
            if format_facts(attachment.pixel_format).depth_or_stencil {
                return Err(invalid("a depth or stencil format as a color"));
            }
            #[expect(unsafe_code, reason = "the read of one color attachment of a pipeline")]
            // SAFETY: the index is below `MAX_COLOR_ATTACHMENTS`, which every Apple GPU has.
            let raw = unsafe { attachments.objectAtIndexedSubscript(index as NSUInteger) };
            raw.setPixelFormat(pixel_format(attachment.pixel_format));
            raw.setWriteMask(write_mask(attachment.write_mask));
            if let Some(blending) = attachment.blending {
                raw.setBlendingEnabled(true);
                raw.setRgbBlendOperation(blend_operation(blending.rgb_blend_operation));
                raw.setAlphaBlendOperation(blend_operation(blending.alpha_blend_operation));
                raw.setSourceRGBBlendFactor(blend_factor(blending.source_rgb_blend_factor));
                raw.setDestinationRGBBlendFactor(blend_factor(
                    blending.destination_rgb_blend_factor,
                ));
                raw.setSourceAlphaBlendFactor(blend_factor(blending.source_alpha_blend_factor));
                raw.setDestinationAlphaBlendFactor(blend_factor(
                    blending.destination_alpha_blend_factor,
                ));
            }
        }
        if let Some(format) = d.depth_attachment_pixel_format {
            raw_descriptor.setDepthAttachmentPixelFormat(pixel_format(format));
        }
        if let Some(format) = d.stencil_attachment_pixel_format {
            raw_descriptor.setStencilAttachmentPixelFormat(pixel_format(format));
        }
        raw_descriptor.setAlphaToCoverageEnabled(d.alpha_to_coverage_enabled);
        raw_descriptor.setRasterizationEnabled(d.rasterization_enabled);
        let raw = self
            .device
            .newRenderPipelineStateWithDescriptor_error(&raw_descriptor)
            .map_err(|error| {
                MakeRenderPipelineStateError::Refused(error.localizedDescription().to_string())
            })?;
        Ok(DirectRenderPipelineState(Rc::new(RenderPipelineState {
            raw,
            vertex: d.vertex_function.0.clone(),
            fragment: d.fragment_function.map(|f| f.0.clone()),
            colors: d.color_attachments.map(|a| a.map(|a| a.pixel_format)),
            depth: d.depth_attachment_pixel_format,
            stencil: d.stencil_attachment_pixel_format,
            sample_count: d.raster_sample_count,
            topology: d.input_primitive_topology,
        })))
    }

    fn make_compute_pipeline_state(
        &mut self,
        function: &DirectFunction,
    ) -> Result<DirectComputePipelineState, MakeComputePipelineStateError> {
        if function.0.stage != naga::ShaderStage::Compute {
            return Err(MakeComputePipelineStateError::NotCompute);
        }
        let raw = self
            .device
            .newComputePipelineStateWithFunction_error(&function.0.raw)
            .map_err(|error| {
                MakeComputePipelineStateError::Refused(error.localizedDescription().to_string())
            })?;
        let [x, y, z] = function.0.reads.workgroup_size;
        if u64::from(x) * u64::from(y) * u64::from(z) > raw.maxTotalThreadsPerThreadgroup() as u64 {
            return Err(MakeComputePipelineStateError::Refused(
                "a workgroup larger than the pipeline runs".into(),
            ));
        }
        let threadgroup: u64 = function.0.reads.threadgroup_lengths.iter().sum();
        if threadgroup > self.device.maxThreadgroupMemoryLength() as u64 {
            return Err(MakeComputePipelineStateError::Refused(
                "workgroup variables larger than the device's threadgroup memory".into(),
            ));
        }
        Ok(DirectComputePipelineState(Rc::new(ComputePipelineState {
            raw,
            function: function.0.clone(),
        })))
    }

    fn make_command_buffer(&mut self) -> Result<DirectCommandBuffer, MakeCommandBufferError> {
        let raw = self
            .queue
            .commandBuffer()
            .ok_or(MakeCommandBufferError::Refused)?;
        Ok(DirectCommandBuffer(Rc::new(CommandBufferState {
            raw,
            committed: Cell::new(false),
        })))
    }

    fn encode_render_commands(
        &mut self,
        command_buffer: &DirectCommandBuffer,
        pass: &RenderPassDescriptor<'_, Self>,
        commands: &[RenderCommand<Self>],
    ) -> Result<(), EncodeRenderCommandsError> {
        if command_buffer.0.committed.get() {
            return Err(EncodeRenderCommandsError::Committed);
        }
        let area = check_pass(pass)?;
        check_render_commands(pass, area, commands)?;
        let descriptor = pass_descriptor(pass);
        let encoder = command_buffer
            .0
            .raw
            .renderCommandEncoderWithDescriptor(&descriptor)
            .ok_or(EncodeRenderCommandsError::Invalid(
                "Metal made no render encoder",
            ))?;
        let mut used = Vec::new();
        if let Some(buffer) = pass.visibility_result_buffer {
            used.push(buffer.clone());
        }
        let mut bound = RenderBindings::default();
        for command in commands {
            encode_render_command(&encoder, &self.empty, &mut bound, command, &mut used);
        }
        encoder.endEncoding();
        for buffer in &used {
            buffer.used_by(command_buffer);
        }
        Ok(())
    }

    fn encode_blit_commands(
        &mut self,
        command_buffer: &DirectCommandBuffer,
        commands: &[BlitCommand<Self>],
    ) -> Result<(), EncodeBlitCommandsError> {
        if command_buffer.0.committed.get() {
            return Err(EncodeBlitCommandsError::Committed);
        }
        for command in commands {
            check_blit(command)?;
        }
        let encoder =
            command_buffer
                .0
                .raw
                .blitCommandEncoder()
                .ok_or(EncodeBlitCommandsError::Invalid(
                    "Metal made no blit encoder",
                ))?;
        for command in commands {
            encode_blit(&encoder, command);
            for buffer in blit_buffers(command) {
                buffer.used_by(command_buffer);
            }
        }
        encoder.endEncoding();
        Ok(())
    }

    fn encode_compute_commands(
        &mut self,
        command_buffer: &DirectCommandBuffer,
        commands: &[ComputeCommand<Self>],
    ) -> Result<(), EncodeComputeCommandsError> {
        if command_buffer.0.committed.get() {
            return Err(EncodeComputeCommandsError::Committed);
        }
        check_compute_commands(commands)?;
        let encoder = command_buffer.0.raw.computeCommandEncoder().ok_or(
            EncodeComputeCommandsError::Invalid("Metal made no compute encoder"),
        )?;
        let mut bound = StageBindings::default();
        let mut pipeline: Option<&DirectComputePipelineState> = None;
        for command in commands {
            match command {
                ComputeCommand::SetComputePipelineState(state) => {
                    encoder.setComputePipelineState(&state.0.raw);
                    pipeline = Some(state);
                }
                ComputeCommand::SetBuffer {
                    buffer,
                    offset,
                    length,
                    index,
                } => {
                    bound.buffers.insert(*index, *length);
                    buffer.used_by(command_buffer);
                    #[expect(unsafe_code, reason = "the binding of a buffer to a compute slot")]
                    // SAFETY: the bound range lies inside the buffer and the slot below `MAX_BUFFER_SLOTS`, both checked by
                    // `check_compute_commands`; the command buffer keeps the buffer.
                    unsafe {
                        encoder.setBuffer_offset_atIndex(
                            Some(&buffer.0.raw),
                            *offset as NSUInteger,
                            *index as NSUInteger,
                        )
                    };
                }
                ComputeCommand::SetBytes { bytes, index } => {
                    bound.buffers.insert(*index, bytes.len() as u64);
                    set_compute_bytes(&encoder, bytes, *index);
                }
                ComputeCommand::SetTexture { texture, index } => {
                    bound.textures.insert(*index, texture.clone());
                    #[expect(unsafe_code, reason = "the binding of a texture to a compute slot")]
                    // SAFETY: the slot is below `MAX_TEXTURE_SLOTS`, checked by `check_compute_commands`.
                    unsafe {
                        encoder.setTexture_atIndex(Some(&texture.0.raw), *index as NSUInteger)
                    };
                }
                ComputeCommand::SetSamplerState { sampler, index } => {
                    bound.samplers.insert(*index, ());
                    #[expect(unsafe_code, reason = "the binding of a sampler to a compute slot")]
                    // SAFETY: the slot is below `MAX_SAMPLER_SLOTS`, checked by `check_compute_commands`.
                    unsafe {
                        encoder.setSamplerState_atIndex(Some(&sampler.0), *index as NSUInteger)
                    };
                }
                ComputeCommand::DispatchThreadgroups(groups) => {
                    let Some(pipeline) = pipeline else {
                        unreachable!("checked by check_compute_commands")
                    };
                    let function = &pipeline.0.function;
                    let sizes = lengths(&function.reads, &bound);
                    if !sizes.is_empty() {
                        set_compute_bytes(&encoder, &sizes, function.reads.sizes_index);
                    }
                    for (index, length) in function.reads.threadgroup_lengths.iter().enumerate() {
                        #[expect(
                            unsafe_code,
                            reason = "the length of one workgroup variable's threadgroup memory"
                        )]
                        // SAFETY: the length is the variable's size rounded up to 16 bytes, all of them together within the
                        // device's threadgroup memory (make_compute_pipeline_state), at the index naga gives the variable.
                        unsafe {
                            encoder.setThreadgroupMemoryLength_atIndex(
                                *length as NSUInteger,
                                index as NSUInteger,
                            )
                        };
                    }
                    let [x, y, z] = function.reads.workgroup_size;
                    encoder.dispatchThreadgroups_threadsPerThreadgroup(
                        mtl_size(*groups),
                        mtl_size(Size {
                            width: x,
                            height: y,
                            depth: z,
                        }),
                    );
                }
            }
        }
        encoder.endEncoding();
        Ok(())
    }

    fn commit(&mut self, command_buffer: &DirectCommandBuffer) -> Result<(), CommitError> {
        if command_buffer.0.committed.replace(true) {
            return Err(CommitError::AlreadyCommitted);
        }
        command_buffer.0.raw.commit();
        Ok(())
    }

    fn wait_until_completed(
        &mut self,
        command_buffer: &DirectCommandBuffer,
    ) -> Result<(), WaitUntilCompletedError> {
        if !command_buffer.0.committed.get() {
            return Err(WaitUntilCompletedError::NotCommitted);
        }
        command_buffer.0.raw.waitUntilCompleted();
        Ok(())
    }

    fn status(&self, command_buffer: &DirectCommandBuffer) -> CommandBufferStatus {
        match command_buffer.0.raw.status() {
            MTLCommandBufferStatus::NotEnqueued => CommandBufferStatus::NotEnqueued,
            MTLCommandBufferStatus::Enqueued => CommandBufferStatus::Enqueued,
            MTLCommandBufferStatus::Committed => CommandBufferStatus::Committed,
            MTLCommandBufferStatus::Scheduled => CommandBufferStatus::Scheduled,
            MTLCommandBufferStatus::Completed => CommandBufferStatus::Completed,
            _ => CommandBufferStatus::Error,
        }
    }

    fn make_layer(&mut self, window: &dyn HasWindowHandle) -> Result<DirectLayer, MakeLayerError> {
        if MainThreadMarker::new().is_none() {
            return Err(MakeLayerError::NotMainThread);
        }
        let handle = window.window_handle().map_err(|_| MakeLayerError::NotAppKit)?;
        let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return Err(MakeLayerError::NotAppKit);
        };
        #[expect(unsafe_code, reason = "the layer of the view a window handle names")]
        // SAFETY: the handle borrows its view, so the pointer is a live NSView during the call, which is on the main thread.
        let layer = unsafe { raw_window_metal::Layer::from_ns_view(appkit.ns_view) };
        #[expect(unsafe_code, reason = "the move of the layer's retain into a typed handle")]
        // SAFETY: `into_raw` gives a retained `CAMetalLayer`, whose one retain the handle takes over.
        let raw = unsafe { Retained::from_raw(layer.into_raw().cast::<CAMetalLayer>().as_ptr()) }
            .ok_or(MakeLayerError::NotAppKit)?;
        raw.setDevice(Some(&self.device));
        Ok(DirectLayer(Rc::new(LayerState {
            raw,
            configured: Cell::new(None),
        })))
    }

    fn configure_layer(
        &mut self,
        layer: &DirectLayer,
        descriptor: &LayerDescriptor,
    ) -> Result<(), ConfigureLayerError> {
        if !matches!(
            descriptor.pixel_format,
            PixelFormat::Bgra8Unorm
                | PixelFormat::Bgra8UnormSrgb
                | PixelFormat::Rgba16Float
                | PixelFormat::Rgb10a2Unorm
        ) {
            return Err(ConfigureLayerError::Invalid("a format no layer takes"));
        }
        if descriptor.width == 0 || descriptor.height == 0 {
            return Err(ConfigureLayerError::Invalid("an empty size"));
        }
        let raw = &layer.0.raw;
        raw.setPixelFormat(pixel_format(descriptor.pixel_format));
        raw.setDrawableSize(CGSize {
            width: f64::from(descriptor.width),
            height: f64::from(descriptor.height),
        });
        raw.setDisplaySyncEnabled(descriptor.display_sync_enabled);
        layer.0.configured.set(Some(*descriptor));
        Ok(())
    }

    fn next_drawable(&mut self, layer: &DirectLayer) -> Result<DirectDrawable, NextDrawableError> {
        let descriptor = layer.0.configured.get().ok_or(NextDrawableError::NotConfigured)?;
        let raw = autoreleasepool(|_| layer.0.raw.nextDrawable()).ok_or(NextDrawableError::TimedOut)?;
        let texture = raw.texture();
        let storage = Retained::as_ptr(&texture).cast::<c_void>() as usize;
        let state = TextureState {
            width: texture.width() as u32,
            height: texture.height() as u32,
            raw: texture,
            texture_type: TextureType::Type2D,
            pixel_format: descriptor.pixel_format,
            depth: 1,
            levels: 1,
            slices: 1,
            sample_count: 1,
            render_target: true,
            pixel_format_view: false,
            storage,
        };
        Ok(DirectDrawable(Rc::new(DrawableState {
            raw,
            texture: DirectTexture(Rc::new(state)),
            presented: Cell::new(false),
        })))
    }

    fn drawable_texture(&self, drawable: &DirectDrawable) -> DirectTexture {
        drawable.0.texture.clone()
    }

    fn present_drawable(
        &mut self,
        command_buffer: &DirectCommandBuffer,
        drawable: &DirectDrawable,
    ) -> Result<(), PresentDrawableError> {
        if command_buffer.0.committed.get() {
            return Err(PresentDrawableError::Committed);
        }
        if drawable.0.presented.replace(true) {
            return Err(PresentDrawableError::AlreadyPresented);
        }
        let shown = ProtocolObject::from_retained(drawable.0.raw.clone());
        command_buffer.0.raw.presentDrawable(&shown);
        Ok(())
    }
}

/// The bytes threadgroup memory lengths are a multiple of.
///
/// https://developer.apple.com/documentation/metal/mtlcomputecommandencoder/setthreadgroupmemorylength(_:index:)
const THREADGROUP_GRANULE: u64 = 16;

/// The faces of one cube.
const CUBE_FACES: u32 = 6;

/// Returns the levels of a full mipmap chain for a largest side.
fn full_chain(side: u32) -> u32 {
    u32::BITS - side.leading_zeros()
}

/// What one stage has bound, by slot.
#[derive(Clone, Default)]
struct StageBindings {
    /// The bytes the bounds checks see at each filled buffer slot.
    buffers: BTreeMap<u32, u64>,
    textures: BTreeMap<u32, DirectTexture>,
    samplers: BTreeMap<u32, ()>,
}

/// What a render pass has bound so far.
#[derive(Default)]
struct RenderBindings {
    vertex: StageBindings,
    fragment: StageBindings,
    pipeline: Option<DirectRenderPipelineState>,
}

/// Returns the bytes past the offset at a slot, for the bounds checks.
fn bytes_past(bound: &StageBindings, slot: u32) -> u64 {
    bound.buffers.get(&slot).copied().unwrap_or(0)
}

/// Returns the buffer of lengths a function's bounds checks read, as bytes, empty when it reads none.
fn lengths(reads: &Reads, bound: &StageBindings) -> Vec<u8> {
    reads
        .sizes
        .iter()
        .flat_map(|size| {
            let length = match size {
                SizeOf::Slot(slot) => bytes_past(bound, *slot),
                SizeOf::Unused => 0,
            };
            u32::try_from(length).unwrap_or(u32::MAX).to_le_bytes()
        })
        .collect()
}

/// Checks that every slot a function reads holds a resource of the kind and size it reads.
fn check_reads(reads: &Reads, bound: &StageBindings) -> Result<(), &'static str> {
    for (slot, least) in &reads.buffers {
        match bound.buffers.get(slot) {
            None => return Err("a buffer slot the function reads is empty"),
            Some(_) if bytes_past(bound, *slot) < *least => {
                return Err("a buffer shorter than what the function reads at its slot");
            }
            Some(_) => {}
        }
    }
    if let Some((slot, least)) = reads.immediates
        && bytes_past(bound, slot) < least
    {
        return Err("immediates shorter than the function's");
    }
    for (slot, read) in &reads.textures {
        let Some(texture) = bound.textures.get(slot) else {
            return Err("a texture slot the function reads is empty");
        };
        if texture.0.texture_type != read.texture_type
            || read.depth && !format_facts(texture.0.pixel_format).depth
        {
            return Err("a texture of another type than the function reads at its slot");
        }
    }
    if reads
        .samplers
        .iter()
        .any(|slot| !bound.samplers.contains_key(slot))
    {
        return Err("a sampler slot the function reads is empty");
    }
    Ok(())
}

/// Checks a pass's attachments and returns its area and sample count.
fn check_pass(
    pass: &RenderPassDescriptor<'_, DirectMetal>,
) -> Result<(u32, u32, u32), EncodeRenderCommandsError> {
    let invalid = EncodeRenderCommandsError::Invalid;
    let mut area: Option<(u32, u32, u32)> = None;
    let mut fit = |texture: &DirectTexture, level: u32, slice: u32, depth_plane: u32| {
        let t = &texture.0;
        if !t.render_target {
            return Err(invalid("an attachment not made to draw into"));
        }
        if level >= t.levels || slice >= t.slices || depth_plane >= (t.depth >> level).max(1) {
            return Err(EncodeRenderCommandsError::OutOfRange(
                "an attachment's level, slice or plane",
            ));
        }
        let size = (
            (t.width >> level).max(1),
            (t.height >> level).max(1),
            t.sample_count,
        );
        match area {
            Some(seen) if seen != size => {
                Err(invalid("attachments of different sizes or sample counts"))
            }
            _ => {
                area = Some(size);
                Ok(())
            }
        }
    };
    for attachment in pass.color_attachments.iter().flatten() {
        if format_facts(attachment.texture.0.pixel_format).depth_or_stencil {
            return Err(invalid("a depth or stencil texture as a color attachment"));
        }
        fit(
            attachment.texture,
            attachment.level,
            attachment.slice,
            attachment.depth_plane,
        )?;
    }
    if let Some(depth) = &pass.depth_attachment {
        if !format_facts(depth.texture.0.pixel_format).depth {
            return Err(invalid("a depth attachment without depth"));
        }
        fit(depth.texture, depth.level, depth.slice, 0)?;
    }
    if let Some(stencil) = &pass.stencil_attachment {
        if !format_facts(stencil.texture.0.pixel_format).stencil {
            return Err(invalid("a stencil attachment without stencil"));
        }
        fit(stencil.texture, stencil.level, stencil.slice, 0)?;
    }
    if let (Some(depth), Some(stencil)) = (&pass.depth_attachment, &pass.stencil_attachment)
        && depth.texture.0.storage != stencil.texture.0.storage
    {
        return Err(invalid(
            "depth and stencil attachments of different textures",
        ));
    }
    for attachment in pass.color_attachments.iter().flatten() {
        if let Some(resolve) = &attachment.resolve {
            let (t, r) = (&attachment.texture.0, &resolve.texture.0);
            if t.sample_count == 1 || r.sample_count != 1 || !r.render_target {
                return Err(invalid("a resolve from or into the wrong sample count"));
            }
            if format_facts(r.pixel_format).bytes != format_facts(t.pixel_format).bytes
                || format_facts(r.pixel_format).srgb_twin != format_facts(t.pixel_format).srgb_twin
                    && r.pixel_format != t.pixel_format
            {
                return Err(invalid("a resolve into another format"));
            }
            let level = resolve.level;
            if level >= r.levels
                || resolve.slice >= r.slices
                || resolve.depth_plane >= (r.depth >> level).max(1)
            {
                return Err(EncodeRenderCommandsError::OutOfRange(
                    "a resolve target's level, slice or plane",
                ));
            }
            if ((r.width >> level).max(1), (r.height >> level).max(1))
                != (
                    (t.width >> attachment.level).max(1),
                    (t.height >> attachment.level).max(1),
                )
            {
                return Err(invalid("a resolve target of another size"));
            }
        }
    }
    if let Some(buffer) = pass.visibility_result_buffer
        && buffer.0.length < VISIBILITY_RESULT_BYTES
    {
        return Err(EncodeRenderCommandsError::OutOfRange(
            "a visibility result buffer",
        ));
    }
    match area {
        Some(area) => Ok(area),
        None if pass.render_target_width > 0
            && pass.render_target_height > 0
            && [1, 2, 4, 8].contains(&pass.default_raster_sample_count) =>
        {
            Ok((
                pass.render_target_width,
                pass.render_target_height,
                pass.default_raster_sample_count,
            ))
        }
        None => Err(invalid("a pass with no attachment and no area")),
    }
}

/// Checks every command of a pass, as it will be encoded, before any is.
fn check_render_commands(
    pass: &RenderPassDescriptor<'_, DirectMetal>,
    (width, height, samples): (u32, u32, u32),
    commands: &[RenderCommand<DirectMetal>],
) -> Result<(), EncodeRenderCommandsError> {
    let invalid = EncodeRenderCommandsError::Invalid;
    let out = EncodeRenderCommandsError::OutOfRange;
    let unbound = EncodeRenderCommandsError::Unbound;
    let colors: Vec<Option<PixelFormat>> = pass
        .color_attachments
        .iter()
        .map(|a| a.as_ref().map(|a| a.texture.0.pixel_format))
        .collect();
    let depth = pass
        .depth_attachment
        .as_ref()
        .map(|a| a.texture.0.pixel_format);
    let stencil = pass
        .stencil_attachment
        .as_ref()
        .map(|a| a.texture.0.pixel_format);
    let mut bound = RenderBindings::default();
    let check_slot = |index: u32, most: u32| {
        if index < most {
            Ok(())
        } else {
            Err(out("a slot past the last"))
        }
    };
    for command in commands {
        match command {
            RenderCommand::SetRenderPipelineState(state) => {
                let s = &state.0;
                if s.colors.as_slice() != colors.as_slice()
                    || s.depth != depth
                    || s.stencil != stencil
                {
                    return Err(invalid("a pipeline made for other attachments"));
                }
                if s.sample_count != samples {
                    return Err(invalid("a pipeline made for another sample count"));
                }
                bound.pipeline = Some(state.clone());
            }
            RenderCommand::SetVertexBuffer {
                buffer,
                offset,
                length,
                index,
            }
            | RenderCommand::SetFragmentBuffer {
                buffer,
                offset,
                length,
                index,
            } => {
                check_slot(*index, MAX_BUFFER_SLOTS)?;
                if !buffer.holds(*offset, *length) {
                    return Err(out("a bound range past its buffer's end"));
                }
                let stage = if matches!(command, RenderCommand::SetVertexBuffer { .. }) {
                    &mut bound.vertex
                } else {
                    &mut bound.fragment
                };
                stage.buffers.insert(*index, *length);
            }
            RenderCommand::SetVertexBytes { bytes, index }
            | RenderCommand::SetFragmentBytes { bytes, index } => {
                check_slot(*index, MAX_BUFFER_SLOTS)?;
                let stage = if matches!(command, RenderCommand::SetVertexBytes { .. }) {
                    &mut bound.vertex
                } else {
                    &mut bound.fragment
                };
                stage.buffers.insert(*index, bytes.len() as u64);
            }
            RenderCommand::SetVertexTexture { texture, index } => {
                check_slot(*index, MAX_TEXTURE_SLOTS)?;
                bound.vertex.textures.insert(*index, texture.clone());
            }
            RenderCommand::SetFragmentTexture { texture, index } => {
                check_slot(*index, MAX_TEXTURE_SLOTS)?;
                bound.fragment.textures.insert(*index, texture.clone());
            }
            RenderCommand::SetVertexSamplerState { index, .. } => {
                check_slot(*index, MAX_SAMPLER_SLOTS)?;
                bound.vertex.samplers.insert(*index, ());
            }
            RenderCommand::SetFragmentSamplerState { index, .. } => {
                check_slot(*index, MAX_SAMPLER_SLOTS)?;
                bound.fragment.samplers.insert(*index, ());
            }
            RenderCommand::SetViewports(viewports) => {
                if viewports.is_empty() || viewports.len() > MAX_VIEWPORTS {
                    return Err(invalid("one to 16 viewports"));
                }
            }
            RenderCommand::SetScissorRects(rects) => {
                if rects.is_empty() || rects.len() > MAX_VIEWPORTS {
                    return Err(invalid("one to 16 scissor rectangles"));
                }
                let inside = |r: &wcontainers_metal::ScissorRect| {
                    r.x.checked_add(r.width).is_some_and(|end| end <= width)
                        && r.y.checked_add(r.height).is_some_and(|end| end <= height)
                };
                if !rects.iter().all(inside) {
                    return Err(out("a scissor rectangle outside the pass"));
                }
            }
            RenderCommand::SetVisibilityResultMode { mode, offset } => {
                let Some(buffer) = pass.visibility_result_buffer else {
                    if *mode == VisibilityResultMode::Disabled {
                        continue;
                    }
                    return Err(invalid("counting with no visibility result buffer"));
                };
                if !offset.is_multiple_of(VISIBILITY_RESULT_BYTES)
                    || !buffer.holds(*offset, VISIBILITY_RESULT_BYTES)
                {
                    return Err(out("a visibility result offset"));
                }
            }
            RenderCommand::DrawPrimitives {
                primitive_type,
                vertex_start,
                vertex_count,
                instance_count,
                base_instance,
            } => {
                if vertex_start.checked_add(*vertex_count).is_none()
                    || base_instance.checked_add(*instance_count).is_none()
                {
                    return Err(out("vertices or instances past the largest index"));
                }
                check_draw(&bound, *primitive_type).map_err(unbound)?;
            }
            RenderCommand::DrawIndexedPrimitives {
                primitive_type,
                index_count,
                index_type,
                index_buffer,
                index_buffer_offset,
                instance_count,
                base_instance,
                ..
            } => {
                let size = match index_type {
                    IndexType::UInt16 => 2,
                    IndexType::UInt32 => 4,
                };
                if !index_buffer_offset.is_multiple_of(size)
                    || !index_buffer.holds(*index_buffer_offset, u64::from(*index_count) * size)
                {
                    return Err(out(
                        "indices outside their buffer, or at an offset not a multiple of their size",
                    ));
                }
                if base_instance.checked_add(*instance_count).is_none() {
                    return Err(out("instances past the largest index"));
                }
                check_draw(&bound, *primitive_type).map_err(unbound)?;
            }
            RenderCommand::SetDepthStencilState(_)
            | RenderCommand::SetCullMode(_)
            | RenderCommand::SetFrontFacingWinding(_)
            | RenderCommand::SetTriangleFillMode(_)
            | RenderCommand::SetDepthClipMode(_)
            | RenderCommand::SetDepthBias { .. }
            | RenderCommand::SetBlendColor(_)
            | RenderCommand::SetStencilReferenceValues { .. } => {}
        }
    }
    Ok(())
}

/// Checks one draw against the bound pipeline and resources.
fn check_draw(bound: &RenderBindings, primitive_type: PrimitiveType) -> Result<(), &'static str> {
    let Some(pipeline) = &bound.pipeline else {
        return Err("a draw with no pipeline");
    };
    let class = match primitive_type {
        PrimitiveType::Point => PrimitiveTopologyClass::Point,
        PrimitiveType::Line | PrimitiveType::LineStrip => PrimitiveTopologyClass::Line,
        PrimitiveType::Triangle | PrimitiveType::TriangleStrip => PrimitiveTopologyClass::Triangle,
    };
    if class != pipeline.0.topology {
        return Err("a draw of primitives the pipeline was not made for");
    }
    check_reads(&pipeline.0.vertex.reads, &bound.vertex)?;
    if let Some(fragment) = &pipeline.0.fragment {
        check_reads(&fragment.reads, &bound.fragment)?;
    }
    Ok(())
}

/// Encodes one checked render command, binding the empty buffer and the lengths a draw's functions read first.
fn encode_render_command(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    empty: &ProtocolObject<dyn MTLBuffer>,
    bound: &mut RenderBindings,
    command: &RenderCommand<DirectMetal>,
    used: &mut Vec<DirectBuffer>,
) {
    match command {
        RenderCommand::SetRenderPipelineState(state) => {
            encoder.setRenderPipelineState(&state.0.raw);
            bound.pipeline = Some(state.clone());
        }
        RenderCommand::SetDepthStencilState(state) => encoder.setDepthStencilState(Some(&state.0)),
        RenderCommand::SetVertexBuffer {
            buffer,
            offset,
            length,
            index,
        } => {
            bound.vertex.buffers.insert(*index, *length);
            used.push(buffer.clone());
            #[expect(unsafe_code, reason = "the binding of a buffer to a vertex slot")]
            // SAFETY: the bound range lies inside the buffer and the slot below `MAX_BUFFER_SLOTS` (check_render_commands);
            // the command buffer keeps the buffer.
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(
                    Some(&buffer.0.raw),
                    *offset as NSUInteger,
                    *index as NSUInteger,
                )
            };
        }
        RenderCommand::SetFragmentBuffer {
            buffer,
            offset,
            length,
            index,
        } => {
            bound.fragment.buffers.insert(*index, *length);
            used.push(buffer.clone());
            #[expect(unsafe_code, reason = "the binding of a buffer to a fragment slot")]
            // SAFETY: as for the vertex slot.
            unsafe {
                encoder.setFragmentBuffer_offset_atIndex(
                    Some(&buffer.0.raw),
                    *offset as NSUInteger,
                    *index as NSUInteger,
                )
            };
        }
        RenderCommand::SetVertexBytes { bytes, index } => {
            bound.vertex.buffers.insert(*index, bytes.len() as u64);
            set_vertex_bytes(encoder, bytes, *index);
        }
        RenderCommand::SetFragmentBytes { bytes, index } => {
            bound.fragment.buffers.insert(*index, bytes.len() as u64);
            set_fragment_bytes(encoder, bytes, *index);
        }
        RenderCommand::SetVertexTexture { texture, index } => {
            bound.vertex.textures.insert(*index, texture.clone());
            #[expect(unsafe_code, reason = "the binding of a texture to a vertex slot")]
            // SAFETY: the slot is below `MAX_TEXTURE_SLOTS` (check_render_commands).
            unsafe {
                encoder.setVertexTexture_atIndex(Some(&texture.0.raw), *index as NSUInteger)
            };
        }
        RenderCommand::SetFragmentTexture { texture, index } => {
            bound.fragment.textures.insert(*index, texture.clone());
            #[expect(unsafe_code, reason = "the binding of a texture to a fragment slot")]
            // SAFETY: the slot is below `MAX_TEXTURE_SLOTS` (check_render_commands).
            unsafe {
                encoder.setFragmentTexture_atIndex(Some(&texture.0.raw), *index as NSUInteger)
            };
        }
        RenderCommand::SetVertexSamplerState { sampler, index } => {
            bound.vertex.samplers.insert(*index, ());
            #[expect(unsafe_code, reason = "the binding of a sampler to a vertex slot")]
            // SAFETY: the slot is below `MAX_SAMPLER_SLOTS` (check_render_commands).
            unsafe {
                encoder.setVertexSamplerState_atIndex(Some(&sampler.0), *index as NSUInteger)
            };
        }
        RenderCommand::SetFragmentSamplerState { sampler, index } => {
            bound.fragment.samplers.insert(*index, ());
            #[expect(unsafe_code, reason = "the binding of a sampler to a fragment slot")]
            // SAFETY: the slot is below `MAX_SAMPLER_SLOTS` (check_render_commands).
            unsafe {
                encoder.setFragmentSamplerState_atIndex(Some(&sampler.0), *index as NSUInteger)
            };
        }
        RenderCommand::SetViewports(viewports) => {
            let raw: Vec<MTLViewport> = viewports
                .iter()
                .map(|v| MTLViewport {
                    originX: v.origin_x,
                    originY: v.origin_y,
                    width: v.width,
                    height: v.height,
                    znear: v.znear,
                    zfar: v.zfar,
                })
                .collect();
            let first = NonNull::from(&raw[0]);
            #[expect(unsafe_code, reason = "the setting of a draw's viewports")]
            // SAFETY: one to `MAX_VIEWPORTS` viewports (check_render_commands), read during the call.
            unsafe {
                encoder.setViewports_count(first, raw.len() as NSUInteger)
            };
        }
        RenderCommand::SetScissorRects(rects) => {
            let raw: Vec<MTLScissorRect> = rects
                .iter()
                .map(|r| MTLScissorRect {
                    x: r.x as NSUInteger,
                    y: r.y as NSUInteger,
                    width: r.width as NSUInteger,
                    height: r.height as NSUInteger,
                })
                .collect();
            let first = NonNull::from(&raw[0]);
            #[expect(unsafe_code, reason = "the setting of a draw's scissor rectangles")]
            // SAFETY: one to `MAX_VIEWPORTS` rectangles inside the pass's area (check_render_commands), read during the
            // call.
            unsafe {
                encoder.setScissorRects_count(first, raw.len() as NSUInteger)
            };
        }
        RenderCommand::SetCullMode(mode) => encoder.setCullMode(match mode {
            CullMode::None => MTLCullMode::None,
            CullMode::Front => MTLCullMode::Front,
            CullMode::Back => MTLCullMode::Back,
        }),
        RenderCommand::SetFrontFacingWinding(winding) => {
            encoder.setFrontFacingWinding(match winding {
                Winding::Clockwise => MTLWinding::Clockwise,
                Winding::CounterClockwise => MTLWinding::CounterClockwise,
            })
        }
        RenderCommand::SetTriangleFillMode(mode) => encoder.setTriangleFillMode(match mode {
            TriangleFillMode::Fill => MTLTriangleFillMode::Fill,
            TriangleFillMode::Lines => MTLTriangleFillMode::Lines,
        }),
        RenderCommand::SetDepthClipMode(mode) => encoder.setDepthClipMode(match mode {
            DepthClipMode::Clip => MTLDepthClipMode::Clip,
            DepthClipMode::Clamp => MTLDepthClipMode::Clamp,
        }),
        RenderCommand::SetDepthBias {
            depth_bias,
            slope_scale,
            clamp,
        } => encoder.setDepthBias_slopeScale_clamp(*depth_bias, *slope_scale, *clamp),
        RenderCommand::SetBlendColor([r, g, b, a]) => {
            encoder.setBlendColorRed_green_blue_alpha(*r, *g, *b, *a)
        }
        RenderCommand::SetStencilReferenceValues { front, back } => {
            encoder.setStencilFrontReferenceValue_backReferenceValue(*front, *back)
        }
        RenderCommand::SetVisibilityResultMode { mode, offset } => encoder
            .setVisibilityResultMode_offset(
                match mode {
                    VisibilityResultMode::Disabled => MTLVisibilityResultMode::Disabled,
                    VisibilityResultMode::Boolean => MTLVisibilityResultMode::Boolean,
                    VisibilityResultMode::Counting => MTLVisibilityResultMode::Counting,
                },
                *offset as NSUInteger,
            ),
        RenderCommand::DrawPrimitives {
            primitive_type,
            vertex_start,
            vertex_count,
            instance_count,
            base_instance,
        } => {
            prepare_draw(encoder, empty, bound);
            #[expect(unsafe_code, reason = "a draw without indices")]
            // SAFETY: every slot the pipeline's functions read holds a resource of the kind and size they read, and they
            // read their vertex inputs, and everything else, bounds-checked against the lengths just bound
            // (check_render_commands, prepare_draw).
            unsafe {
                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                    primitive_type_of(*primitive_type),
                    *vertex_start as NSUInteger,
                    *vertex_count as NSUInteger,
                    *instance_count as NSUInteger,
                    *base_instance as NSUInteger,
                )
            };
        }
        RenderCommand::DrawIndexedPrimitives {
            primitive_type,
            index_count,
            index_type,
            index_buffer,
            index_buffer_offset,
            instance_count,
            base_vertex,
            base_instance,
        } => {
            prepare_draw(encoder, empty, bound);
            used.push(index_buffer.clone());
            #[expect(unsafe_code, reason = "a draw with indices")]
            // SAFETY: as for a draw without indices; and the indices lie inside their buffer at an offset that is a
            // multiple of their size (check_render_commands), while the vertices they name are read bounds-checked.
            unsafe {
                encoder
                    .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount_baseVertex_baseInstance(
                        primitive_type_of(*primitive_type),
                        *index_count as NSUInteger,
                        match index_type {
                            IndexType::UInt16 => MTLIndexType::UInt16,
                            IndexType::UInt32 => MTLIndexType::UInt32,
                        },
                        &index_buffer.0.raw,
                        *index_buffer_offset as NSUInteger,
                        *instance_count as NSUInteger,
                        *base_vertex as isize,
                        *base_instance as NSUInteger,
                    )
            };
        }
    }
}

/// Binds the empty buffer to every vertex buffer slot no command filled, and the lengths each function's checks read.
fn prepare_draw(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    empty: &ProtocolObject<dyn MTLBuffer>,
    bound: &mut RenderBindings,
) {
    let Some(pipeline) = bound.pipeline.clone() else {
        unreachable!("checked by check_render_commands")
    };
    let vertex = &pipeline.0.vertex.reads;
    for slot in &vertex.vertex_buffers {
        if !bound.vertex.buffers.contains_key(slot) {
            #[expect(
                unsafe_code,
                reason = "the binding of the empty buffer to a vertex buffer slot"
            )]
            // SAFETY: the slot is below `MAX_BUFFER_SLOTS` (translation), and the function reads its length as zero.
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(Some(empty), 0, *slot as NSUInteger)
            };
        }
    }
    let sizes = lengths(vertex, &bound.vertex);
    if !sizes.is_empty() {
        set_vertex_bytes(encoder, &sizes, vertex.sizes_index);
    }
    if let Some(fragment) = &pipeline.0.fragment {
        let sizes = lengths(&fragment.reads, &bound.fragment);
        if !sizes.is_empty() {
            set_fragment_bytes(encoder, &sizes, fragment.reads.sizes_index);
        }
    }
}

/// Copies bytes into the command buffer for a vertex slot.
fn set_vertex_bytes(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    bytes: &[u8],
    index: u32,
) {
    let Some(first) =
        NonNull::new(bytes.as_ptr().cast_mut().cast::<c_void>()).filter(|_| !bytes.is_empty())
    else {
        return;
    };
    #[expect(unsafe_code, reason = "the copy of bytes into a vertex slot")]
    // SAFETY: the bytes are read during the call, and the slot is below `MAX_BUFFER_SLOTS`.
    unsafe {
        encoder.setVertexBytes_length_atIndex(first, bytes.len() as NSUInteger, index as NSUInteger)
    };
}

/// Copies bytes into the command buffer for a fragment slot.
fn set_fragment_bytes(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    bytes: &[u8],
    index: u32,
) {
    let Some(first) =
        NonNull::new(bytes.as_ptr().cast_mut().cast::<c_void>()).filter(|_| !bytes.is_empty())
    else {
        return;
    };
    #[expect(unsafe_code, reason = "the copy of bytes into a fragment slot")]
    // SAFETY: the bytes are read during the call, and the slot is below `MAX_BUFFER_SLOTS`.
    unsafe {
        encoder.setFragmentBytes_length_atIndex(
            first,
            bytes.len() as NSUInteger,
            index as NSUInteger,
        )
    };
}

/// Copies bytes into the command buffer for a compute slot.
fn set_compute_bytes(
    encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
    bytes: &[u8],
    index: u32,
) {
    let Some(first) =
        NonNull::new(bytes.as_ptr().cast_mut().cast::<c_void>()).filter(|_| !bytes.is_empty())
    else {
        return;
    };
    #[expect(unsafe_code, reason = "the copy of bytes into a compute slot")]
    // SAFETY: the bytes are read during the call, and the slot is below `MAX_BUFFER_SLOTS`.
    unsafe {
        encoder.setBytes_length_atIndex(first, bytes.len() as NSUInteger, index as NSUInteger)
    };
}

/// Checks every compute command before any is encoded.
fn check_compute_commands(
    commands: &[ComputeCommand<DirectMetal>],
) -> Result<(), EncodeComputeCommandsError> {
    let out = EncodeComputeCommandsError::OutOfRange;
    let mut bound = StageBindings::default();
    let mut pipeline: Option<&DirectComputePipelineState> = None;
    let check_slot = |index: u32, most: u32| {
        if index < most {
            Ok(())
        } else {
            Err(out("a slot past the last"))
        }
    };
    for command in commands {
        match command {
            ComputeCommand::SetComputePipelineState(state) => pipeline = Some(state),
            ComputeCommand::SetBuffer {
                buffer,
                offset,
                length,
                index,
            } => {
                check_slot(*index, MAX_BUFFER_SLOTS)?;
                if !buffer.holds(*offset, *length) {
                    return Err(out("a bound range past its buffer's end"));
                }
                bound.buffers.insert(*index, *length);
            }
            ComputeCommand::SetBytes { bytes, index } => {
                check_slot(*index, MAX_BUFFER_SLOTS)?;
                bound.buffers.insert(*index, bytes.len() as u64);
            }
            ComputeCommand::SetTexture { texture, index } => {
                check_slot(*index, MAX_TEXTURE_SLOTS)?;
                bound.textures.insert(*index, texture.clone());
            }
            ComputeCommand::SetSamplerState { index, .. } => {
                check_slot(*index, MAX_SAMPLER_SLOTS)?;
                bound.samplers.insert(*index, ());
            }
            ComputeCommand::DispatchThreadgroups(_) => {
                let Some(pipeline) = pipeline else {
                    return Err(EncodeComputeCommandsError::Unbound(
                        "a dispatch with no pipeline",
                    ));
                };
                check_reads(&pipeline.0.function.reads, &bound)
                    .map_err(EncodeComputeCommandsError::Unbound)?;
            }
        }
    }
    Ok(())
}

/// Returns the buffers a copy or fill uses.
fn blit_buffers(command: &BlitCommand<DirectMetal>) -> Vec<&DirectBuffer> {
    match command {
        BlitCommand::CopyFromBufferToBuffer {
            source_buffer,
            destination_buffer,
            ..
        } => {
            vec![source_buffer, destination_buffer]
        }
        BlitCommand::CopyFromBufferToTexture { source_buffer, .. } => vec![source_buffer],
        BlitCommand::CopyFromTextureToBuffer {
            destination_buffer, ..
        } => vec![destination_buffer],
        BlitCommand::CopyFromTextureToTexture { .. } => Vec::new(),
        BlitCommand::FillBuffer { buffer, .. } => vec![buffer],
    }
}

/// Returns the bytes and the block of what a copy between a buffer and a texture moves, by its option.
fn moved(format: PixelFormat, option: BlitOption) -> Option<(u64, (u32, u32))> {
    let facts = format_facts(format);
    match (option, format) {
        (BlitOption::None, PixelFormat::Depth32FloatStencil8) => None,
        (BlitOption::None, _) => Some((u64::from(facts.bytes), facts.block)),
        (BlitOption::DepthFromDepthStencil, PixelFormat::Depth32FloatStencil8) => Some((4, (1, 1))),
        (BlitOption::StencilFromDepthStencil, PixelFormat::Depth32FloatStencil8) => {
            Some((1, (1, 1)))
        }
        _ => None,
    }
}

/// Checks a texture region of one slice and level: inside the level, in whole blocks, and one slice unless 3D.
fn check_region(
    texture: &DirectTexture,
    slice: u32,
    level: u32,
    origin: Origin,
    size: Size,
    block: (u32, u32),
) -> Result<(), &'static str> {
    let t = &texture.0;
    if level >= t.levels || slice >= t.slices {
        return Err("a level or slice outside the texture");
    }
    let (w, h, d) = (
        (t.width >> level).max(1),
        (t.height >> level).max(1),
        if t.texture_type == TextureType::Type3D {
            (t.depth >> level).max(1)
        } else {
            1
        },
    );
    let fits = |at: u32, n: u32, most: u32| at.checked_add(n).is_some_and(|end| end <= most);
    // A region may end at the level's edge inside a block, which counts as whole.
    let (bw, bh) = block;
    let rounded = |n: u32, b: u32| n.next_multiple_of(b);
    if !fits(origin.x, size.width, rounded(w, bw))
        || !fits(origin.y, size.height, rounded(h, bh))
        || !fits(origin.z, size.depth, d)
    {
        return Err("a region outside the level");
    }
    let edge = |at: u32, n: u32, most: u32, b: u32| {
        at.is_multiple_of(b) && (n.is_multiple_of(b) || at + n == most)
    };
    if !edge(origin.x, size.width, w, bw) || !edge(origin.y, size.height, h, bh) {
        return Err("a region of partial blocks");
    }
    if size.width == 0 || size.height == 0 || size.depth == 0 {
        return Err("an empty region");
    }
    Ok(())
}

/// Checks one copy or fill.
fn check_blit(command: &BlitCommand<DirectMetal>) -> Result<(), EncodeBlitCommandsError> {
    let out = EncodeBlitCommandsError::OutOfRange;
    let invalid = EncodeBlitCommandsError::Invalid;
    match command {
        BlitCommand::CopyFromBufferToBuffer {
            source_buffer,
            source_offset,
            destination_buffer,
            destination_offset,
            size,
        } => {
            if !source_buffer.holds(*source_offset, *size)
                || !destination_buffer.holds(*destination_offset, *size)
            {
                return Err(out("a copy outside its buffer"));
            }
            if Rc::ptr_eq(&source_buffer.0, &destination_buffer.0)
                && source_offset < &(destination_offset + size)
                && destination_offset < &(source_offset + size)
            {
                return Err(invalid("a copy between overlapping ranges"));
            }
        }
        BlitCommand::CopyFromBufferToTexture {
            source_buffer,
            source_offset,
            source_bytes_per_row,
            source_bytes_per_image,
            source_size,
            destination_texture: texture,
            destination_slice: slice,
            destination_level: level,
            destination_origin: origin,
            options,
        }
        | BlitCommand::CopyFromTextureToBuffer {
            source_texture: texture,
            source_slice: slice,
            source_level: level,
            source_origin: origin,
            source_size,
            destination_buffer: source_buffer,
            destination_offset: source_offset,
            destination_bytes_per_row: source_bytes_per_row,
            destination_bytes_per_image: source_bytes_per_image,
            options,
        } => {
            if texture.0.sample_count != 1 {
                return Err(invalid(
                    "a copy between a buffer and a multisampled texture",
                ));
            }
            let Some((bytes, block)) = moved(texture.0.pixel_format, *options) else {
                return Err(invalid(
                    "a copy of an aspect the format does not have, or of both at once",
                ));
            };
            check_region(texture, *slice, *level, *origin, *source_size, block).map_err(out)?;
            let blocks_wide = u64::from(source_size.width.div_ceil(block.0));
            let rows = u64::from(source_size.height.div_ceil(block.1));
            let row = blocks_wide * bytes;
            if *source_bytes_per_row < row || !source_bytes_per_row.is_multiple_of(bytes) {
                return Err(invalid(
                    "a row pitch shorter than a row or not a multiple of a block",
                ));
            }
            let image = (rows - 1) * source_bytes_per_row + row;
            let depth = u64::from(source_size.depth);
            if depth > 1 && *source_bytes_per_image < rows * source_bytes_per_row {
                return Err(invalid("an image pitch shorter than an image"));
            }
            if !source_offset.is_multiple_of(bytes) {
                return Err(invalid("a buffer offset not a multiple of a block"));
            }
            let length = (depth - 1) * source_bytes_per_image + image;
            if !source_buffer.holds(*source_offset, length) {
                return Err(out("a copy outside its buffer"));
            }
        }
        BlitCommand::CopyFromTextureToTexture {
            source_texture,
            source_slice,
            source_level,
            source_origin,
            source_size,
            destination_texture,
            destination_slice,
            destination_level,
            destination_origin,
        } => {
            let (s, d) = (&source_texture.0, &destination_texture.0);
            let (sf, df) = (format_facts(s.pixel_format), format_facts(d.pixel_format));
            let same = s.pixel_format == d.pixel_format
                || sf.srgb_twin == Some(d.pixel_format)
                || !sf.depth_or_stencil
                    && !df.depth_or_stencil
                    && sf.bytes == df.bytes
                    && sf.block == df.block;
            if !same || s.sample_count != d.sample_count {
                return Err(invalid(
                    "a copy between textures of formats or sample counts that differ",
                ));
            }
            check_region(
                source_texture,
                *source_slice,
                *source_level,
                *source_origin,
                *source_size,
                sf.block,
            )
            .map_err(out)?;
            check_region(
                destination_texture,
                *destination_slice,
                *destination_level,
                *destination_origin,
                *source_size,
                df.block,
            )
            .map_err(out)?;
            let apart = |a: u32, b: u32, n: u32| a + n <= b || b + n <= a;
            if s.storage == d.storage
                && source_level == destination_level
                && source_slice == destination_slice
                && !(apart(source_origin.x, destination_origin.x, source_size.width)
                    || apart(source_origin.y, destination_origin.y, source_size.height)
                    || apart(source_origin.z, destination_origin.z, source_size.depth))
            {
                return Err(invalid("a copy between overlapping regions"));
            }
        }
        BlitCommand::FillBuffer {
            buffer,
            offset,
            length,
            ..
        } => {
            if !buffer.holds(*offset, *length) {
                return Err(out("a fill outside its buffer"));
            }
        }
    }
    Ok(())
}

/// Returns Metal's origin of an origin.
fn mtl_origin(origin: Origin) -> MTLOrigin {
    MTLOrigin {
        x: origin.x as NSUInteger,
        y: origin.y as NSUInteger,
        z: origin.z as NSUInteger,
    }
}

/// Returns Metal's size of a size.
fn mtl_size(size: Size) -> MTLSize {
    MTLSize {
        width: size.width as NSUInteger,
        height: size.height as NSUInteger,
        depth: size.depth as NSUInteger,
    }
}

/// Returns Metal's option of a copy option.
fn blit_option(option: BlitOption) -> MTLBlitOption {
    match option {
        BlitOption::None => MTLBlitOption::empty(),
        BlitOption::DepthFromDepthStencil => MTLBlitOption::DepthFromDepthStencil,
        BlitOption::StencilFromDepthStencil => MTLBlitOption::StencilFromDepthStencil,
    }
}

/// Encodes one checked copy or fill.
fn encode_blit(
    encoder: &ProtocolObject<dyn MTLBlitCommandEncoder>,
    command: &BlitCommand<DirectMetal>,
) {
    match command {
        BlitCommand::CopyFromBufferToBuffer {
            source_buffer,
            source_offset,
            destination_buffer,
            destination_offset,
            size,
        } => {
            #[expect(unsafe_code, reason = "a copy between buffers")]
            // SAFETY: both ranges lie inside their buffers and do not overlap (check_blit).
            unsafe {
                encoder.copyFromBuffer_sourceOffset_toBuffer_destinationOffset_size(
                    &source_buffer.0.raw,
                    *source_offset as NSUInteger,
                    &destination_buffer.0.raw,
                    *destination_offset as NSUInteger,
                    *size as NSUInteger,
                )
            };
        }
        BlitCommand::CopyFromBufferToTexture {
            source_buffer,
            source_offset,
            source_bytes_per_row,
            source_bytes_per_image,
            source_size,
            destination_texture,
            destination_slice,
            destination_level,
            destination_origin,
            options,
        } => {
            #[expect(unsafe_code, reason = "a copy from a buffer into a texture")]
            // SAFETY: the region lies inside the level in whole blocks, the bytes inside the buffer at pitches of whole
            // blocks, and the option names an aspect the format has (check_blit).
            unsafe {
                encoder.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin_options(
                    &source_buffer.0.raw, *source_offset as NSUInteger, *source_bytes_per_row as NSUInteger,
                    *source_bytes_per_image as NSUInteger, mtl_size(*source_size), &destination_texture.0.raw,
                    *destination_slice as NSUInteger, *destination_level as NSUInteger, mtl_origin(*destination_origin),
                    blit_option(*options),
                )
            };
        }
        BlitCommand::CopyFromTextureToBuffer {
            source_texture,
            source_slice,
            source_level,
            source_origin,
            source_size,
            destination_buffer,
            destination_offset,
            destination_bytes_per_row,
            destination_bytes_per_image,
            options,
        } => {
            #[expect(unsafe_code, reason = "a copy from a texture into a buffer")]
            // SAFETY: as for a copy into a texture (check_blit).
            unsafe {
                encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage_options(
                    &source_texture.0.raw, *source_slice as NSUInteger, *source_level as NSUInteger,
                    mtl_origin(*source_origin), mtl_size(*source_size), &destination_buffer.0.raw,
                    *destination_offset as NSUInteger, *destination_bytes_per_row as NSUInteger,
                    *destination_bytes_per_image as NSUInteger, blit_option(*options),
                )
            };
        }
        BlitCommand::CopyFromTextureToTexture {
            source_texture,
            source_slice,
            source_level,
            source_origin,
            source_size,
            destination_texture,
            destination_slice,
            destination_level,
            destination_origin,
        } => {
            #[expect(unsafe_code, reason = "a copy between textures")]
            // SAFETY: both regions lie inside their levels in whole blocks, of formats and sample counts that agree, and
            // do not overlap (check_blit).
            unsafe {
                encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                    &source_texture.0.raw, *source_slice as NSUInteger, *source_level as NSUInteger,
                    mtl_origin(*source_origin), mtl_size(*source_size), &destination_texture.0.raw,
                    *destination_slice as NSUInteger, *destination_level as NSUInteger, mtl_origin(*destination_origin),
                )
            };
        }
        BlitCommand::FillBuffer {
            buffer,
            offset,
            length,
            value,
        } => {
            encoder.fillBuffer_range_value(
                &buffer.0.raw,
                NSRange::new(*offset as usize, *length as usize),
                *value,
            );
        }
    }
}

/// Builds Metal's pass descriptor of a checked pass.
fn pass_descriptor(
    pass: &RenderPassDescriptor<'_, DirectMetal>,
) -> Retained<MTLRenderPassDescriptor> {
    let raw = MTLRenderPassDescriptor::new();
    let attachments = raw.colorAttachments();
    for (index, attachment) in pass.color_attachments.iter().enumerate() {
        let Some(a) = attachment else {
            continue;
        };
        #[expect(unsafe_code, reason = "the read of one color attachment of a pass")]
        // SAFETY: the index is below `MAX_COLOR_ATTACHMENTS`, which every Apple GPU has.
        let r = unsafe { attachments.objectAtIndexedSubscript(index as NSUInteger) };
        r.setTexture(Some(&a.texture.0.raw));
        r.setLevel(a.level as NSUInteger);
        r.setSlice(a.slice as NSUInteger);
        r.setDepthPlane(a.depth_plane as NSUInteger);
        r.setLoadAction(load_action(a.load_action));
        let [red, green, blue, alpha] = a.clear_color;
        r.setClearColor(MTLClearColor {
            red,
            green,
            blue,
            alpha,
        });
        match &a.resolve {
            Some(resolve) => {
                r.setResolveTexture(Some(&resolve.texture.0.raw));
                r.setResolveLevel(resolve.level as NSUInteger);
                r.setResolveSlice(resolve.slice as NSUInteger);
                r.setResolveDepthPlane(resolve.depth_plane as NSUInteger);
                r.setStoreAction(match a.store_action {
                    StoreAction::Store => MTLStoreAction::StoreAndMultisampleResolve,
                    StoreAction::DontCare => MTLStoreAction::MultisampleResolve,
                });
            }
            None => r.setStoreAction(store_action(a.store_action)),
        }
    }
    if let Some(a) = &pass.depth_attachment {
        let r = raw.depthAttachment();
        r.setTexture(Some(&a.texture.0.raw));
        r.setLevel(a.level as NSUInteger);
        r.setSlice(a.slice as NSUInteger);
        r.setLoadAction(load_action(a.load_action));
        r.setStoreAction(store_action(a.store_action));
        r.setClearDepth(a.clear_value);
    }
    if let Some(a) = &pass.stencil_attachment {
        let r = raw.stencilAttachment();
        r.setTexture(Some(&a.texture.0.raw));
        r.setLevel(a.level as NSUInteger);
        r.setSlice(a.slice as NSUInteger);
        r.setLoadAction(load_action(a.load_action));
        r.setStoreAction(store_action(a.store_action));
        r.setClearStencil(a.clear_value as u32);
    }
    if let Some(buffer) = pass.visibility_result_buffer {
        raw.setVisibilityResultBuffer(Some(&buffer.0.raw));
    }
    let empty = pass.color_attachments.iter().all(Option::is_none)
        && pass.depth_attachment.is_none()
        && pass.stencil_attachment.is_none();
    if empty {
        raw.setRenderTargetWidth(pass.render_target_width as NSUInteger);
        raw.setRenderTargetHeight(pass.render_target_height as NSUInteger);
        raw.setDefaultRasterSampleCount(pass.default_raster_sample_count as NSUInteger);
    }
    raw
}

/// Returns naga's options for a function, its pipeline options, and what the function will read.
fn translation(
    source: &FunctionSource<'_>,
) -> Result<
    (
        naga::back::msl::Options,
        naga::back::msl::PipelineOptions,
        Reads,
    ),
    MakeFunctionError,
> {
    let unchecked = |why: &str| MakeFunctionError::Unchecked(why.into());
    let module = source.module;
    let entry_index = module
        .entry_points
        .iter()
        .position(|e| e.name == source.entry_point && e.stage == source.stage)
        .ok_or_else(|| unchecked("no such entry point"))?;
    let entry = &module.entry_points[entry_index];
    if !matches!(
        source.stage,
        naga::ShaderStage::Vertex | naga::ShaderStage::Fragment | naga::ShaderStage::Compute
    ) {
        return Err(unchecked("a stage other than vertex, fragment and compute"));
    }
    let info = source.info.get_entry_point(entry_index);
    let reserved = [source.immediates_buffer_index, source.sizes_buffer_index];
    if reserved.iter().any(|&slot| slot >= MAX_BUFFER_SLOTS) || reserved[0] == reserved[1] {
        return Err(unchecked(
            "the immediates and the lengths need two buffer slots below 31",
        ));
    }
    let mut resources = naga::back::msl::BindingMap::default();
    let mut reads = Reads {
        sizes_index: source.sizes_buffer_index,
        workgroup_size: entry.workgroup_size,
        ..Reads::default()
    };
    let mut buffer_slots = Vec::new();
    for (handle, var) in module.global_variables.iter() {
        let used = !info[handle].is_empty();
        let ty = &module.types[var.ty].inner;
        if ty.needs_host_buffer_byte_size(&module.types) {
            let slot = var.binding.and_then(|b| {
                source
                    .resources
                    .iter()
                    .find(|r| r.group == b.group && r.binding == b.binding)
            });
            reads.sizes.push(match slot {
                Some(slot) if used => SizeOf::Slot(slot.index),
                _ => SizeOf::Unused,
            });
        }
        if !used {
            continue;
        }
        match var.space {
            naga::AddressSpace::Immediate => {
                reads.immediates = Some((
                    source.immediates_buffer_index,
                    u64::from(ty.size(module.to_ctx())),
                ));
                continue;
            }
            naga::AddressSpace::WorkGroup => {
                let length =
                    u64::from(ty.size(module.to_ctx())).next_multiple_of(THREADGROUP_GRANULE);
                reads.threadgroup_lengths.push(length);
                continue;
            }
            naga::AddressSpace::Private | naga::AddressSpace::Function => continue,
            naga::AddressSpace::Uniform
            | naga::AddressSpace::Storage { .. }
            | naga::AddressSpace::Handle => {}
            _ => {
                return Err(unchecked(
                    "a global in an address space this crate does not check",
                ));
            }
        }
        let Some(binding) = var.binding else {
            return Err(unchecked("a resource with no binding"));
        };
        let Some(slot) = source
            .resources
            .iter()
            .find(|r| r.group == binding.group && r.binding == binding.binding)
        else {
            return Err(unchecked("a resource with no slot"));
        };
        let index = slot.index;
        let mut target = naga::back::msl::BindTarget::default();
        match (var.space, ty) {
            (naga::AddressSpace::Uniform | naga::AddressSpace::Storage { .. }, _) => {
                if index >= MAX_BUFFER_SLOTS
                    || reserved.contains(&index)
                    || source
                        .vertex_buffers
                        .iter()
                        .any(|v| v.buffer_index == index)
                {
                    return Err(unchecked("a buffer slot past 30 or taken"));
                }
                let least = match *ty {
                    naga::TypeInner::Struct { ref members, .. }
                        if ty.needs_host_buffer_byte_size(&module.types) =>
                    {
                        u64::from(members.last().map_or(0, |m| m.offset))
                    }
                    naga::TypeInner::Array {
                        size: naga::ArraySize::Dynamic,
                        ..
                    } => 0,
                    naga::TypeInner::BindingArray { .. } => {
                        return Err(unchecked("an array of bindings"));
                    }
                    _ => u64::from(ty.size(module.to_ctx())),
                };
                if let naga::AddressSpace::Storage { access } = var.space {
                    target.mutable = access.contains(naga::StorageAccess::STORE);
                }
                target.buffer = Some(slot_number(index));
                buffer_slots.push(index);
                reads.buffers.push((index, least));
            }
            (
                _,
                naga::TypeInner::Image {
                    dim,
                    arrayed,
                    class,
                },
            ) => {
                if index >= MAX_TEXTURE_SLOTS {
                    return Err(unchecked("a texture slot past 127"));
                }
                let (multisampled, depth) = match *class {
                    naga::ImageClass::Sampled { multi, .. } => (multi, false),
                    naga::ImageClass::Depth { multi } => (multi, true),
                    naga::ImageClass::Storage { access, .. } => {
                        if access.contains(naga::StorageAccess::STORE)
                            || access.contains(naga::StorageAccess::ATOMIC)
                        {
                            return Err(unchecked(
                                "a texture the function writes, whose writes naga does not bound",
                            ));
                        }
                        (false, false)
                    }
                    _ => return Err(unchecked("an external texture")),
                };
                let texture_type = match (dim, arrayed, multisampled) {
                    (naga::ImageDimension::D2, false, false) => TextureType::Type2D,
                    (naga::ImageDimension::D2, false, true) => TextureType::Type2DMultisample,
                    (naga::ImageDimension::D2, true, false) => TextureType::Type2DArray,
                    (naga::ImageDimension::Cube, false, false) => TextureType::TypeCube,
                    (naga::ImageDimension::Cube, true, false) => TextureType::TypeCubeArray,
                    (naga::ImageDimension::D3, false, false) => TextureType::Type3D,
                    _ => return Err(unchecked("a texture type this crate does not make")),
                };
                target.texture = Some(slot_number(index));
                reads.textures.push((
                    index,
                    TextureRead {
                        texture_type,
                        depth,
                    },
                ));
            }
            (_, naga::TypeInner::Sampler { .. }) => {
                if index >= MAX_SAMPLER_SLOTS {
                    return Err(unchecked("a sampler slot past 15"));
                }
                target.sampler = Some(naga::back::msl::BindSamplerTarget::Resource(slot_number(
                    index,
                )));
                reads.samplers.push(index);
            }
            _ => return Err(unchecked("a resource of a type this crate does not check")),
        }
        resources.insert(binding, target);
    }
    let mut mappings = Vec::new();
    if source.stage == naga::ShaderStage::Vertex {
        for layout in source.vertex_buffers {
            if layout.buffer_index >= MAX_BUFFER_SLOTS
                || reserved.contains(&layout.buffer_index)
                || buffer_slots.contains(&layout.buffer_index)
            {
                return Err(unchecked("a vertex buffer slot past 30 or taken"));
            }
            if layout.stride == 0 {
                return Err(unchecked("a vertex buffer of stride zero"));
            }
            let mut attributes = Vec::new();
            for attribute in &layout.attributes {
                let (format, bytes) = vertex_format(attribute.format);
                if attribute
                    .offset
                    .checked_add(bytes)
                    .is_none_or(|end| end > layout.stride)
                {
                    return Err(unchecked("an attribute past its vertex buffer's stride"));
                }
                attributes.push(naga::back::msl::AttributeMapping {
                    shader_location: attribute.location,
                    offset: attribute.offset,
                    format,
                });
            }
            mappings.push(naga::back::msl::VertexBufferMapping {
                id: layout.buffer_index,
                stride: layout.stride,
                step_mode: match layout.step_function {
                    VertexStepFunction::Constant => naga::back::msl::VertexBufferStepMode::Constant,
                    VertexStepFunction::PerVertex => {
                        naga::back::msl::VertexBufferStepMode::ByVertex
                    }
                    VertexStepFunction::PerInstance => {
                        naga::back::msl::VertexBufferStepMode::ByInstance
                    }
                },
                attributes,
            });
            reads.vertex_buffers.push(layout.buffer_index);
            reads.sizes.push(SizeOf::Slot(layout.buffer_index));
        }
    }
    let restrict = naga::proc::BoundsCheckPolicy::Restrict;
    let options = naga::back::msl::Options {
        lang_version: LANGUAGE_VERSION,
        per_entry_point_map: naga::back::msl::EntryPointResourceMap::from([(
            source.entry_point.to_owned(),
            naga::back::msl::EntryPointResources {
                resources,
                immediates_buffer: Some(slot_number(source.immediates_buffer_index)),
                sizes_buffer: Some(slot_number(source.sizes_buffer_index)),
            },
        )]),
        inline_samplers: Vec::new(),
        spirv_cross_compatibility: false,
        fake_missing_bindings: false,
        bounds_check_policies: naga::proc::BoundsCheckPolicies {
            index: restrict,
            buffer: restrict,
            image_load: restrict,
            binding_array: restrict,
        },
        zero_initialize_workgroup_memory: true,
        force_loop_bounding: true,
        task_dispatch_limits: None,
        mesh_shader_primitive_indices_clamp: true,
        emit_int_div_checks: true,
        ray_query_initialization_tracking: true,
    };
    let pipeline = naga::back::msl::PipelineOptions {
        entry_point: Some((source.stage, source.entry_point.to_owned())),
        allow_and_force_point_size: source.point_size,
        vertex_pulling_transform: source.stage == naga::ShaderStage::Vertex,
        vertex_buffer_mappings: mappings,
        binding_array_length_map: Default::default(),
    };
    Ok((options, pipeline, reads))
}

/// Returns a slot as naga's slot type; every slot is below 128, checked before.
fn slot_number(index: u32) -> naga::back::msl::Slot {
    naga::back::msl::Slot::try_from(index).unwrap_or(naga::back::msl::Slot::MAX)
}

/// Returns naga's vertex format of a Metal vertex format, and its bytes.
fn vertex_format(format: VertexFormat) -> (naga_types::VertexFormat, u32) {
    use naga_types::VertexFormat as N;
    match format {
        VertexFormat::UChar2 => (N::Uint8x2, 2),
        VertexFormat::UChar4 => (N::Uint8x4, 4),
        VertexFormat::Char2 => (N::Sint8x2, 2),
        VertexFormat::Char4 => (N::Sint8x4, 4),
        VertexFormat::UChar2Normalized => (N::Unorm8x2, 2),
        VertexFormat::UChar4Normalized => (N::Unorm8x4, 4),
        VertexFormat::Char2Normalized => (N::Snorm8x2, 2),
        VertexFormat::Char4Normalized => (N::Snorm8x4, 4),
        VertexFormat::UShort2 => (N::Uint16x2, 4),
        VertexFormat::UShort4 => (N::Uint16x4, 8),
        VertexFormat::Short2 => (N::Sint16x2, 4),
        VertexFormat::Short4 => (N::Sint16x4, 8),
        VertexFormat::UShort2Normalized => (N::Unorm16x2, 4),
        VertexFormat::UShort4Normalized => (N::Unorm16x4, 8),
        VertexFormat::Short2Normalized => (N::Snorm16x2, 4),
        VertexFormat::Short4Normalized => (N::Snorm16x4, 8),
        VertexFormat::Half2 => (N::Float16x2, 4),
        VertexFormat::Half4 => (N::Float16x4, 8),
        VertexFormat::Float => (N::Float32, 4),
        VertexFormat::Float2 => (N::Float32x2, 8),
        VertexFormat::Float3 => (N::Float32x3, 12),
        VertexFormat::Float4 => (N::Float32x4, 16),
        VertexFormat::UInt => (N::Uint32, 4),
        VertexFormat::UInt2 => (N::Uint32x2, 8),
        VertexFormat::UInt3 => (N::Uint32x3, 12),
        VertexFormat::UInt4 => (N::Uint32x4, 16),
        VertexFormat::Int => (N::Sint32, 4),
        VertexFormat::Int2 => (N::Sint32x2, 8),
        VertexFormat::Int3 => (N::Sint32x3, 12),
        VertexFormat::Int4 => (N::Sint32x4, 16),
        VertexFormat::UChar => (N::Uint8, 1),
        VertexFormat::Char => (N::Sint8, 1),
        VertexFormat::UCharNormalized => (N::Unorm8, 1),
        VertexFormat::CharNormalized => (N::Snorm8, 1),
        VertexFormat::UShort => (N::Uint16, 2),
        VertexFormat::Short => (N::Sint16, 2),
        VertexFormat::UShortNormalized => (N::Unorm16, 2),
        VertexFormat::ShortNormalized => (N::Snorm16, 2),
        VertexFormat::Half => (N::Float16, 2),
        VertexFormat::UInt1010102Normalized => (N::Unorm10_10_10_2, 4),
        VertexFormat::UChar4NormalizedBgra => (N::Unorm8x4Bgra, 4),
    }
}

/// What one pixel format is, for the checks.
#[derive(Clone, Copy)]
struct FormatFacts {
    /// Bytes of one block, a texel for formats that are not compressed.
    bytes: u32,
    block: (u32, u32),
    depth: bool,
    stencil: bool,
    depth_or_stencil: bool,
    /// The same bits seen as sRGB or as linear values, for formats that have such a twin.
    srgb_twin: Option<PixelFormat>,
}

/// Returns what a pixel format is.
fn format_facts(format: PixelFormat) -> FormatFacts {
    use PixelFormat as P;
    let plain = |bytes| FormatFacts {
        bytes,
        block: (1, 1),
        depth: false,
        stencil: false,
        depth_or_stencil: false,
        srgb_twin: None,
    };
    let twin = |bytes, other| FormatFacts {
        srgb_twin: Some(other),
        ..plain(bytes)
    };
    let blocks = |bytes, (w, h), other: Option<PixelFormat>| FormatFacts {
        block: (w, h),
        srgb_twin: other,
        ..plain(bytes)
    };
    let aspects = |bytes, depth, stencil| FormatFacts {
        depth,
        stencil,
        depth_or_stencil: true,
        ..plain(bytes)
    };
    match format {
        P::A8Unorm | P::R8Snorm | P::R8Uint | P::R8Sint => plain(1),
        P::R8Unorm => twin(1, P::R8UnormSrgb),
        P::R8UnormSrgb => twin(1, P::R8Unorm),
        P::R16Unorm | P::R16Snorm | P::R16Uint | P::R16Sint | P::R16Float => plain(2),
        P::Rg8Unorm | P::Rg8Snorm | P::Rg8Uint | P::Rg8Sint => plain(2),
        P::B5g6r5Unorm | P::A1bgr5Unorm | P::Abgr4Unorm | P::Bgr5a1Unorm => plain(2),
        P::R32Uint | P::R32Sint | P::R32Float => plain(4),
        P::Rg16Unorm | P::Rg16Snorm | P::Rg16Uint | P::Rg16Sint | P::Rg16Float => plain(4),
        P::Rgba8Unorm => twin(4, P::Rgba8UnormSrgb),
        P::Rgba8UnormSrgb => twin(4, P::Rgba8Unorm),
        P::Bgra8Unorm => twin(4, P::Bgra8UnormSrgb),
        P::Bgra8UnormSrgb => twin(4, P::Bgra8Unorm),
        P::Rgba8Snorm | P::Rgba8Uint | P::Rgba8Sint => plain(4),
        P::Rgb10a2Unorm | P::Rgb10a2Uint | P::Rg11b10Float | P::Rgb9e5Float => plain(4),
        P::Rg32Uint | P::Rg32Sint | P::Rg32Float => plain(8),
        P::Rgba16Unorm | P::Rgba16Snorm | P::Rgba16Uint | P::Rgba16Sint | P::Rgba16Float => {
            plain(8)
        }
        P::Rgba32Uint | P::Rgba32Sint | P::Rgba32Float => plain(16),
        P::Bc1Rgba => blocks(8, (4, 4), Some(P::Bc1RgbaSrgb)),
        P::Bc1RgbaSrgb => blocks(8, (4, 4), Some(P::Bc1Rgba)),
        P::Bc2Rgba => blocks(16, (4, 4), Some(P::Bc2RgbaSrgb)),
        P::Bc2RgbaSrgb => blocks(16, (4, 4), Some(P::Bc2Rgba)),
        P::Bc3Rgba => blocks(16, (4, 4), Some(P::Bc3RgbaSrgb)),
        P::Bc3RgbaSrgb => blocks(16, (4, 4), Some(P::Bc3Rgba)),
        P::Bc4RUnorm | P::Bc4RSnorm => blocks(8, (4, 4), None),
        P::Bc5RgUnorm | P::Bc5RgSnorm | P::Bc6hRgbFloat | P::Bc6hRgbUfloat => {
            blocks(16, (4, 4), None)
        }
        P::Bc7RgbaUnorm => blocks(16, (4, 4), Some(P::Bc7RgbaUnormSrgb)),
        P::Bc7RgbaUnormSrgb => blocks(16, (4, 4), Some(P::Bc7RgbaUnorm)),
        P::Etc2Rgb8 => blocks(8, (4, 4), Some(P::Etc2Rgb8Srgb)),
        P::Etc2Rgb8Srgb => blocks(8, (4, 4), Some(P::Etc2Rgb8)),
        P::EacRgba8 => blocks(16, (4, 4), Some(P::EacRgba8Srgb)),
        P::EacRgba8Srgb => blocks(16, (4, 4), Some(P::EacRgba8)),
        P::Astc4x4Ldr => blocks(16, (4, 4), Some(P::Astc4x4Srgb)),
        P::Astc4x4Srgb => blocks(16, (4, 4), Some(P::Astc4x4Ldr)),
        P::Depth16Unorm => aspects(2, true, false),
        P::Depth32Float => aspects(4, true, false),
        P::Stencil8 => aspects(1, false, true),
        P::Depth32FloatStencil8 => aspects(8, true, true),
        P::X32Stencil8 => aspects(8, false, true),
    }
}

/// Returns Metal's pixel format of a pixel format.
fn pixel_format(format: PixelFormat) -> MTLPixelFormat {
    use PixelFormat as P;
    match format {
        P::A8Unorm => MTLPixelFormat::A8Unorm,
        P::R8Unorm => MTLPixelFormat::R8Unorm,
        P::R8UnormSrgb => MTLPixelFormat::R8Unorm_sRGB,
        P::R8Snorm => MTLPixelFormat::R8Snorm,
        P::R8Uint => MTLPixelFormat::R8Uint,
        P::R8Sint => MTLPixelFormat::R8Sint,
        P::R16Unorm => MTLPixelFormat::R16Unorm,
        P::R16Snorm => MTLPixelFormat::R16Snorm,
        P::R16Uint => MTLPixelFormat::R16Uint,
        P::R16Sint => MTLPixelFormat::R16Sint,
        P::R16Float => MTLPixelFormat::R16Float,
        P::Rg8Unorm => MTLPixelFormat::RG8Unorm,
        P::Rg8Snorm => MTLPixelFormat::RG8Snorm,
        P::Rg8Uint => MTLPixelFormat::RG8Uint,
        P::Rg8Sint => MTLPixelFormat::RG8Sint,
        P::B5g6r5Unorm => MTLPixelFormat::B5G6R5Unorm,
        P::A1bgr5Unorm => MTLPixelFormat::A1BGR5Unorm,
        P::Abgr4Unorm => MTLPixelFormat::ABGR4Unorm,
        P::Bgr5a1Unorm => MTLPixelFormat::BGR5A1Unorm,
        P::R32Uint => MTLPixelFormat::R32Uint,
        P::R32Sint => MTLPixelFormat::R32Sint,
        P::R32Float => MTLPixelFormat::R32Float,
        P::Rg16Unorm => MTLPixelFormat::RG16Unorm,
        P::Rg16Snorm => MTLPixelFormat::RG16Snorm,
        P::Rg16Uint => MTLPixelFormat::RG16Uint,
        P::Rg16Sint => MTLPixelFormat::RG16Sint,
        P::Rg16Float => MTLPixelFormat::RG16Float,
        P::Rgba8Unorm => MTLPixelFormat::RGBA8Unorm,
        P::Rgba8UnormSrgb => MTLPixelFormat::RGBA8Unorm_sRGB,
        P::Rgba8Snorm => MTLPixelFormat::RGBA8Snorm,
        P::Rgba8Uint => MTLPixelFormat::RGBA8Uint,
        P::Rgba8Sint => MTLPixelFormat::RGBA8Sint,
        P::Bgra8Unorm => MTLPixelFormat::BGRA8Unorm,
        P::Bgra8UnormSrgb => MTLPixelFormat::BGRA8Unorm_sRGB,
        P::Rgb10a2Unorm => MTLPixelFormat::RGB10A2Unorm,
        P::Rgb10a2Uint => MTLPixelFormat::RGB10A2Uint,
        P::Rg11b10Float => MTLPixelFormat::RG11B10Float,
        P::Rgb9e5Float => MTLPixelFormat::RGB9E5Float,
        P::Rg32Uint => MTLPixelFormat::RG32Uint,
        P::Rg32Sint => MTLPixelFormat::RG32Sint,
        P::Rg32Float => MTLPixelFormat::RG32Float,
        P::Rgba16Unorm => MTLPixelFormat::RGBA16Unorm,
        P::Rgba16Snorm => MTLPixelFormat::RGBA16Snorm,
        P::Rgba16Uint => MTLPixelFormat::RGBA16Uint,
        P::Rgba16Sint => MTLPixelFormat::RGBA16Sint,
        P::Rgba16Float => MTLPixelFormat::RGBA16Float,
        P::Rgba32Uint => MTLPixelFormat::RGBA32Uint,
        P::Rgba32Sint => MTLPixelFormat::RGBA32Sint,
        P::Rgba32Float => MTLPixelFormat::RGBA32Float,
        P::Bc1Rgba => MTLPixelFormat::BC1_RGBA,
        P::Bc1RgbaSrgb => MTLPixelFormat::BC1_RGBA_sRGB,
        P::Bc2Rgba => MTLPixelFormat::BC2_RGBA,
        P::Bc2RgbaSrgb => MTLPixelFormat::BC2_RGBA_sRGB,
        P::Bc3Rgba => MTLPixelFormat::BC3_RGBA,
        P::Bc3RgbaSrgb => MTLPixelFormat::BC3_RGBA_sRGB,
        P::Bc4RUnorm => MTLPixelFormat::BC4_RUnorm,
        P::Bc4RSnorm => MTLPixelFormat::BC4_RSnorm,
        P::Bc5RgUnorm => MTLPixelFormat::BC5_RGUnorm,
        P::Bc5RgSnorm => MTLPixelFormat::BC5_RGSnorm,
        P::Bc6hRgbFloat => MTLPixelFormat::BC6H_RGBFloat,
        P::Bc6hRgbUfloat => MTLPixelFormat::BC6H_RGBUfloat,
        P::Bc7RgbaUnorm => MTLPixelFormat::BC7_RGBAUnorm,
        P::Bc7RgbaUnormSrgb => MTLPixelFormat::BC7_RGBAUnorm_sRGB,
        P::Etc2Rgb8 => MTLPixelFormat::ETC2_RGB8,
        P::Etc2Rgb8Srgb => MTLPixelFormat::ETC2_RGB8_sRGB,
        P::EacRgba8 => MTLPixelFormat::EAC_RGBA8,
        P::EacRgba8Srgb => MTLPixelFormat::EAC_RGBA8_sRGB,
        P::Astc4x4Ldr => MTLPixelFormat::ASTC_4x4_LDR,
        P::Astc4x4Srgb => MTLPixelFormat::ASTC_4x4_sRGB,
        P::Depth16Unorm => MTLPixelFormat::Depth16Unorm,
        P::Depth32Float => MTLPixelFormat::Depth32Float,
        P::Stencil8 => MTLPixelFormat::Stencil8,
        P::Depth32FloatStencil8 => MTLPixelFormat::Depth32Float_Stencil8,
        P::X32Stencil8 => MTLPixelFormat::X32_Stencil8,
    }
}

/// Returns Metal's texture type of a texture type.
fn texture_type(texture_type: TextureType) -> MTLTextureType {
    match texture_type {
        TextureType::Type2D => MTLTextureType::Type2D,
        TextureType::Type2DArray => MTLTextureType::Type2DArray,
        TextureType::Type2DMultisample => MTLTextureType::Type2DMultisample,
        TextureType::TypeCube => MTLTextureType::TypeCube,
        TextureType::TypeCubeArray => MTLTextureType::TypeCubeArray,
        TextureType::Type3D => MTLTextureType::Type3D,
    }
}

/// Returns Metal's filter of a filter.
fn min_mag_filter(filter: SamplerMinMagFilter) -> MTLSamplerMinMagFilter {
    match filter {
        SamplerMinMagFilter::Nearest => MTLSamplerMinMagFilter::Nearest,
        SamplerMinMagFilter::Linear => MTLSamplerMinMagFilter::Linear,
    }
}

/// Returns Metal's address mode of an address mode.
fn address_mode(mode: SamplerAddressMode) -> MTLSamplerAddressMode {
    match mode {
        SamplerAddressMode::ClampToEdge => MTLSamplerAddressMode::ClampToEdge,
        SamplerAddressMode::MirrorClampToEdge => MTLSamplerAddressMode::MirrorClampToEdge,
        SamplerAddressMode::Repeat => MTLSamplerAddressMode::Repeat,
        SamplerAddressMode::MirrorRepeat => MTLSamplerAddressMode::MirrorRepeat,
        SamplerAddressMode::ClampToZero => MTLSamplerAddressMode::ClampToZero,
        SamplerAddressMode::ClampToBorderColor => MTLSamplerAddressMode::ClampToBorderColor,
    }
}

/// Returns Metal's comparison of a comparison.
fn compare_function(compare: CompareFunction) -> MTLCompareFunction {
    match compare {
        CompareFunction::Never => MTLCompareFunction::Never,
        CompareFunction::Less => MTLCompareFunction::Less,
        CompareFunction::Equal => MTLCompareFunction::Equal,
        CompareFunction::LessEqual => MTLCompareFunction::LessEqual,
        CompareFunction::Greater => MTLCompareFunction::Greater,
        CompareFunction::NotEqual => MTLCompareFunction::NotEqual,
        CompareFunction::GreaterEqual => MTLCompareFunction::GreaterEqual,
        CompareFunction::Always => MTLCompareFunction::Always,
    }
}

/// Returns Metal's stencil operation of a stencil operation.
fn stencil_operation(operation: StencilOperation) -> MTLStencilOperation {
    match operation {
        StencilOperation::Keep => MTLStencilOperation::Keep,
        StencilOperation::Zero => MTLStencilOperation::Zero,
        StencilOperation::Replace => MTLStencilOperation::Replace,
        StencilOperation::IncrementClamp => MTLStencilOperation::IncrementClamp,
        StencilOperation::DecrementClamp => MTLStencilOperation::DecrementClamp,
        StencilOperation::Invert => MTLStencilOperation::Invert,
        StencilOperation::IncrementWrap => MTLStencilOperation::IncrementWrap,
        StencilOperation::DecrementWrap => MTLStencilOperation::DecrementWrap,
    }
}

/// Returns Metal's blend factor of a blend factor.
fn blend_factor(factor: BlendFactor) -> MTLBlendFactor {
    match factor {
        BlendFactor::Zero => MTLBlendFactor::Zero,
        BlendFactor::One => MTLBlendFactor::One,
        BlendFactor::SourceColor => MTLBlendFactor::SourceColor,
        BlendFactor::OneMinusSourceColor => MTLBlendFactor::OneMinusSourceColor,
        BlendFactor::SourceAlpha => MTLBlendFactor::SourceAlpha,
        BlendFactor::OneMinusSourceAlpha => MTLBlendFactor::OneMinusSourceAlpha,
        BlendFactor::DestinationColor => MTLBlendFactor::DestinationColor,
        BlendFactor::OneMinusDestinationColor => MTLBlendFactor::OneMinusDestinationColor,
        BlendFactor::DestinationAlpha => MTLBlendFactor::DestinationAlpha,
        BlendFactor::OneMinusDestinationAlpha => MTLBlendFactor::OneMinusDestinationAlpha,
        BlendFactor::SourceAlphaSaturated => MTLBlendFactor::SourceAlphaSaturated,
        BlendFactor::BlendColor => MTLBlendFactor::BlendColor,
        BlendFactor::OneMinusBlendColor => MTLBlendFactor::OneMinusBlendColor,
        BlendFactor::BlendAlpha => MTLBlendFactor::BlendAlpha,
        BlendFactor::OneMinusBlendAlpha => MTLBlendFactor::OneMinusBlendAlpha,
        BlendFactor::Source1Color => MTLBlendFactor::Source1Color,
        BlendFactor::OneMinusSource1Color => MTLBlendFactor::OneMinusSource1Color,
        BlendFactor::Source1Alpha => MTLBlendFactor::Source1Alpha,
        BlendFactor::OneMinusSource1Alpha => MTLBlendFactor::OneMinusSource1Alpha,
    }
}

/// Returns Metal's blend operation of a blend operation.
fn blend_operation(operation: BlendOperation) -> MTLBlendOperation {
    match operation {
        BlendOperation::Add => MTLBlendOperation::Add,
        BlendOperation::Subtract => MTLBlendOperation::Subtract,
        BlendOperation::ReverseSubtract => MTLBlendOperation::ReverseSubtract,
        BlendOperation::Min => MTLBlendOperation::Min,
        BlendOperation::Max => MTLBlendOperation::Max,
    }
}

/// Returns Metal's write mask of a write mask.
fn write_mask(mask: wcontainers_metal::ColorWriteMask) -> MTLColorWriteMask {
    let mut raw = MTLColorWriteMask::empty();
    for (kept, flag) in [
        (mask.red, MTLColorWriteMask::Red),
        (mask.green, MTLColorWriteMask::Green),
        (mask.blue, MTLColorWriteMask::Blue),
        (mask.alpha, MTLColorWriteMask::Alpha),
    ] {
        if kept {
            raw |= flag;
        }
    }
    raw
}

/// Returns Metal's load action of a load action.
fn load_action(action: LoadAction) -> MTLLoadAction {
    match action {
        LoadAction::DontCare => MTLLoadAction::DontCare,
        LoadAction::Load => MTLLoadAction::Load,
        LoadAction::Clear => MTLLoadAction::Clear,
    }
}

/// Returns Metal's store action of a store action, for an attachment not resolved.
fn store_action(action: StoreAction) -> MTLStoreAction {
    match action {
        StoreAction::DontCare => MTLStoreAction::DontCare,
        StoreAction::Store => MTLStoreAction::Store,
    }
}

/// Returns Metal's primitive type of a primitive type.
fn primitive_type_of(primitive_type: PrimitiveType) -> MTLPrimitiveType {
    match primitive_type {
        PrimitiveType::Point => MTLPrimitiveType::Point,
        PrimitiveType::Line => MTLPrimitiveType::Line,
        PrimitiveType::LineStrip => MTLPrimitiveType::LineStrip,
        PrimitiveType::Triangle => MTLPrimitiveType::Triangle,
        PrimitiveType::TriangleStrip => MTLPrimitiveType::TriangleStrip,
    }
}
