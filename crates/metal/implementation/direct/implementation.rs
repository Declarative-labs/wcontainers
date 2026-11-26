//! Metal's own calls through objc2-metal, each checked against what it touches just before it is made.
//!
//! Functions are written by naga with every access bounds-checked, so a draw is sound once each slot it reads holds a
//! resource of the right kind and size, which each encode checks.
#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::c_void;
use std::fmt;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use naga::proc::Alignment;
use objc2::MainThreadMarker;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSRange, NSString, NSUInteger};
use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLBlitCommandEncoder, MTLBlitOption, MTLBlitPassDescriptor,
    MTLBuffer, MTLClearColor, MTLColorWriteMask, MTLCommandBuffer, MTLCommandBufferStatus,
    MTLCommandEncoder, MTLCommandQueue, MTLCompareFunction, MTLCompileOptions,
    MTLComputeCommandEncoder, MTLComputePipelineState, MTLCounterDontSample,
    MTLCounterSampleBuffer, MTLCounterSampleBufferDescriptor, MTLCounterSamplingPoint,
    MTLCounterSet, MTLCreateSystemDefaultDevice, MTLCullMode, MTLDepthClipMode,
    MTLDepthStencilDescriptor, MTLDepthStencilState, MTLDevice, MTLEvent, MTLFunction,
    MTLGPUFamily, MTLIndexType, MTLLanguageVersion, MTLLibrary, MTLLoadAction, MTLOrigin,
    MTLPixelFormat, MTLPrimitiveTopologyClass, MTLPrimitiveType, MTLRenderCommandEncoder,
    MTLRenderPassDescriptor, MTLRenderPipelineDescriptor, MTLRenderPipelineState,
    MTLResourceOptions, MTLSamplerAddressMode, MTLSamplerBorderColor, MTLSamplerDescriptor,
    MTLSamplerMinMagFilter, MTLSamplerMipFilter, MTLSamplerState, MTLScissorRect, MTLSize,
    MTLStencilDescriptor, MTLStencilOperation, MTLStorageMode, MTLStoreAction, MTLTexture,
    MTLTextureDescriptor, MTLTextureSwizzle, MTLTextureSwizzleChannels, MTLTextureType,
    MTLTextureUsage, MTLTriangleFillMode, MTLViewport, MTLVisibilityResultMode,
    MTLVisibilityResultType, MTLWinding,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use wcontainers_metal::{
    BlendFactor, BlendOperation, BlitCommand, BlitOption, CommandBufferStatus, CommitError,
    CompareFunction, ComputeCommand, ConfigureLayerError, CullMode, DISPATCH_ARGUMENTS_BYTES,
    DRAW_ARGUMENTS_BYTES, DRAW_INDEXED_ARGUMENTS_BYTES, DepthClipMode, DepthStencilDescriptor,
    EncodeBlitCommandsError, EncodeComputeCommandsError, EncodeRenderCommandsError,
    EncodeTimestampError, FunctionSource, INDIRECT_ARGUMENTS_ALIGNMENT, IndexType, LayerDescriptor,
    LoadAction, MAX_ANISOTROPY, MAX_BUFFER_SLOTS, MAX_COLOR_ATTACHMENTS, MAX_SAMPLER_SLOTS,
    MAX_TEXTURE_SIDE, MAX_TEXTURE_SLOTS, MAX_TIMESTAMPS, MAX_VERTEX_STRIDE, MAX_VIEWPORTS,
    MakeBufferError, MakeCommandBufferError, MakeComputePipelineStateError,
    MakeDepthStencilStateError, MakeFunctionError, MakeLayerError, MakeRenderPipelineStateError,
    MakeSamplerStateError, MakeTextureError, MakeTextureViewError, MakeTimestampBufferError, Metal,
    NextDrawableError, Origin, PixelFormat, PresentDrawableError, PrimitiveTopologyClass,
    PrimitiveType, ReadContentsError, ReadTimestampsError, RenderCommand, RenderPassDescriptor,
    RenderPipelineColorAttachmentDescriptor, RenderPipelineDescriptor, SamplerAddressMode,
    SamplerBorderColor, SamplerDescriptor, SamplerMinMagFilter, SamplerMipFilter, Size,
    StandaloneMetal, StencilDescriptor, StencilOperation, StoreAction, TextureDescriptor,
    TextureSwizzle, TextureSwizzleChannels, TextureType, TextureViewDescriptor, TriangleFillMode,
    VISIBILITY_RESULT_BYTES, VertexFormat, VertexStepFunction, VisibilityResultMode,
    WaitUntilCompletedError, Winding, WriteContentsError,
};

/// The Metal Shading Language version functions are written in and compiled as.
///
/// https://developer.apple.com/documentation/metal/mtllanguageversion/version3_1
const LANGUAGE_VERSION: (u8, u8) = (3, 1);

/// The bytes of the buffer bound to vertex buffer slots no command filled, whose length the function reads as zero.
///
/// It holds one element of the longest stride, which Metal's validation wants a vertex buffer to hold.
///
/// https://developer.apple.com/documentation/metal/mtldevice/makebuffer(length:options:)
const EMPTY_BUFFER_BYTES: usize = MAX_VERTEX_STRIDE as usize;

/// The longest side of a 2D, cube or multisampled texture on GPUs of the Apple7 to Apple9 families.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const MAX_SIDE: u32 = 16_384;

/// The longest side of a 3D texture.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const MAX_3D_SIDE: u32 = 2048;

/// The most layers of an array texture, cube faces of a cube array counted one each.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const MAX_LAYERS: u32 = 2048;

/// The bytes of the visibility result buffer a pass counts into from GPUs of the Apple7 family on.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const VISIBILITY_RESULT_SPAN: u64 = 262_144;

/// The bytes buffer copies and fills on macOS take their offsets and sizes in multiples of.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/fill(buffer:range:value:)
const BUFFER_COPY_GRANULE: u64 = 4;

/// Returns whether the check at a site refuses its call; with the `reach` feature it counts the outcome at the site.
///
/// The site is a constant of `site`, which must be listed in `site::ALL`: a constant declared outside `sites!` fails
/// the build here.
macro_rules! refuses {
    (site::$site:ident, $refused:expr) => {{
        const {
            assert!(
                crate::site::listed(crate::site::$site),
                "a site not listed in site::ALL"
            )
        };
        let refused: bool = $refused;
        #[cfg(feature = "reach")]
        crate::reach::add_outcome(crate::site::$site, refused);
        refused
    }};
}

/// Returns what a check found, its call refused when it found nothing; with the `reach` feature it counts the outcome.
///
/// The site is a constant of `site`, listed in `site::ALL` as `refuses!` asserts.
macro_rules! found {
    (site::$site:ident, $found:expr) => {{
        const {
            assert!(
                crate::site::listed(crate::site::$site),
                "a site not listed in site::ALL"
            )
        };
        let found = $found;
        #[cfg(feature = "reach")]
        crate::reach::add_outcome(crate::site::$site, found.is_none());
        found
    }};
}

/// How often each check refused and passed its calls, for the fuzzer's measure of how far it reaches.
#[cfg(feature = "reach")]
pub mod reach {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, PoisonError};

    /// How often one check refused its calls and passed them.
    #[derive(Clone, Copy, Default)]
    pub struct Outcomes {
        pub refused: u64,
        pub passed: u64,
    }

    /// The outcomes counted at each site.
    static OUTCOMES: Mutex<BTreeMap<&'static str, Outcomes>> = Mutex::new(BTreeMap::new());

    /// Adds one to the refusals or the passes of a site.
    pub(crate) fn add_outcome(site: &'static str, refused: bool) {
        let mut outcomes = OUTCOMES.lock().unwrap_or_else(PoisonError::into_inner);
        let counts = outcomes.entry(site).or_default();
        if refused {
            counts.refused = counts.refused.saturating_add(1);
        } else {
            counts.passed = counts.passed.saturating_add(1);
        }
    }

    /// Returns the outcomes counted at each site so far.
    pub fn outcomes() -> BTreeMap<&'static str, Outcomes> {
        OUTCOMES
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Returns every site's constant name and label, as the compiler lists them in `site::ALL`.
    pub fn sites() -> &'static [(&'static str, &'static str)] {
        crate::site::ALL
    }
}

/// The labels of the checks, each naming the call and what its check refuses.
///
/// The fuzzer's reach table names each site by its label. Every label is declared by `sites!`, which lists it in `ALL`
/// from the same entry, so that no site is declared and left out of the list.
mod site {
    /// Declares each entry's label as a constant and lists every entry, by name and label, in `ALL`.
    macro_rules! sites {
        ($($(#[$doc:meta])* const $name:ident: &str = $label:literal;)*) => {
            $($(#[$doc])* pub(crate) const $name: &str = $label;)*

            /// The name and label of every site, in the order they are declared.
            pub(crate) const ALL: &[(&str, &str)] = &[$((stringify!($name), $name)),*];
        };
    }

    /// Returns whether two byte strings are equal, as a constant function can compare them.
    const fn same(left: &[u8], right: &[u8]) -> bool {
        let (mut left, mut right) = (left, right);
        loop {
            match (left, right) {
                ([first, left_rest @ ..], [other, right_rest @ ..]) => {
                    if *first != *other {
                        return false;
                    }
                    (left, right) = (left_rest, right_rest);
                }
                ([], []) => return true,
                _ => return false,
            }
        }
    }

    /// Returns whether a label is one of `sites`' labels.
    const fn listed_in(sites: &[(&str, &str)], label: &str) -> bool {
        let mut sites = sites;
        while let [(_, first), rest @ ..] = sites {
            if same(first.as_bytes(), label.as_bytes()) {
                return true;
            }
            sites = rest;
        }
        false
    }

    /// Returns whether a label is listed in `ALL`; each check asserts it of its site while it is compiled.
    pub(crate) const fn listed(label: &str) -> bool {
        listed_in(ALL, label)
    }

    sites! {
        /// The check of pipeline that refuses a vertex function of another stage.
        const PIPELINE_VERTEX_FUNCTION_OF_ANOTHER_STAGE: &str = "pipeline: a vertex function of another stage";
        /// The check of pipeline that refuses a fragment function of another stage.
        const PIPELINE_FRAGMENT_FUNCTION_OF_ANOTHER_STAGE: &str = "pipeline: a fragment function of another stage";
        /// The check of pipeline that refuses a sample count the device lacks.
        const PIPELINE_SAMPLE_COUNT_DEVICE_LACKS: &str = "pipeline: a sample count the device lacks";
        /// The check of pipeline that refuses a depth or stencil format without the aspect.
        const PIPELINE_DEPTH_OR_STENCIL_FORMAT_WITHOUT_ASPECT: &str = "pipeline: a depth or stencil format without the aspect";
        /// The check of pipeline that refuses a combined depth and stencil format beside another.
        const PIPELINE_COMBINED_DEPTH_AND_STENCIL_FORMAT_BESIDE_ANOTHER: &str = "pipeline: a combined depth and stencil format beside another";
        /// The check of pipeline that refuses a second source beside other attachments.
        const PIPELINE_SECOND_SOURCE_BESIDE_OTHER_ATTACHMENTS: &str = "pipeline: a second source beside other attachments";
        /// The check of pipeline that refuses a color format no attachment takes.
        const PIPELINE_COLOR_FORMAT_NO_ATTACHMENT_TAKES: &str = "pipeline: a color format no attachment takes";
        /// The check of pipeline that refuses some channels of a shared exponent.
        const PIPELINE_SOME_CHANNELS_OF_SHARED_EXPONENT: &str = "pipeline: some channels of a shared exponent";
        /// The check of pipeline that refuses blending a format that does not blend.
        const PIPELINE_BLENDING_FORMAT_THAT_DOES_NOT_BLEND: &str = "pipeline: blending a format that does not blend";
        /// The check of pipeline that refuses alpha to coverage without a blending first attachment.
        const PIPELINE_ALPHA_TO_COVERAGE_WITHOUT_BLENDING_FIRST_ATTACHMENT: &str = "pipeline: alpha to coverage without a blending first attachment";
        /// The check of buffer that refuses longer than the device's most.
        const BUFFER_LONGER_THAN_DEVICES_MOST: &str = "buffer: longer than the device's most";
        /// The check of write that refuses past the buffer.
        const WRITE_PAST_BUFFER: &str = "write: past the buffer";
        /// The check of write that refuses a buffer uncommitted work uses.
        const WRITE_BUFFER_UNCOMMITTED_WORK_USES: &str = "write: a buffer uncommitted work uses";
        /// The check of read that refuses past the buffer.
        const READ_PAST_BUFFER: &str = "read: past the buffer";
        /// The check of read that refuses a buffer uncommitted work uses.
        const READ_BUFFER_UNCOMMITTED_WORK_USES: &str = "read: a buffer uncommitted work uses";
        /// The check of texture that refuses a side past the most.
        const TEXTURE_SIDE_PAST_MOST: &str = "texture: a side past the most";
        /// The check of texture that refuses layers past the most.
        const TEXTURE_LAYERS_PAST_MOST: &str = "texture: layers past the most";
        /// The check of texture that refuses a stencil view format.
        const TEXTURE_STENCIL_VIEW_FORMAT: &str = "texture: a stencil view format";
        /// The check of texture that refuses an empty size.
        const TEXTURE_EMPTY_SIZE: &str = "texture: an empty size";
        /// The check of texture that refuses deeper than one outside 3D.
        const TEXTURE_DEEPER_THAN_ONE_OUTSIDE_3D: &str = "texture: deeper than one outside 3D";
        /// The check of texture that refuses an array length the type does not take.
        const TEXTURE_ARRAY_LENGTH_TYPE_DOES_NOT_TAKE: &str = "texture: an array length the type does not take";
        /// The check of texture that refuses a cube not square.
        const TEXTURE_CUBE_NOT_SQUARE: &str = "texture: a cube not square";
        /// The check of texture that refuses levels past the chain.
        const TEXTURE_LEVELS_PAST_CHAIN: &str = "texture: levels past the chain";
        /// The check of texture that refuses a sample count the type does not take.
        const TEXTURE_SAMPLE_COUNT_TYPE_DOES_NOT_TAKE: &str = "texture: a sample count the type does not take";
        /// The check of texture that refuses compressed in a type that is not.
        const TEXTURE_COMPRESSED_IN_TYPE_THAT_IS_NOT: &str = "texture: compressed in a type that is not";
        /// The check of texture that refuses depth or stencil in 3D.
        const TEXTURE_DEPTH_OR_STENCIL_IN_3D: &str = "texture: depth or stencil in 3D";
        /// The check of view that refuses of a drawable's texture.
        const VIEW_OF_DRAWABLES_TEXTURE: &str = "view: of a drawable's texture";
        /// The check of view that refuses levels or slices past the texture.
        const VIEW_LEVELS_OR_SLICES_PAST_TEXTURE: &str = "view: levels or slices past the texture";
        /// The check of view that refuses a format the texture is not seen as.
        const VIEW_FORMAT_TEXTURE_IS_NOT_SEEN_AS: &str = "view: a format the texture is not seen as";
        /// The check of view that refuses slices the type does not take.
        const VIEW_SLICES_TYPE_DOES_NOT_TAKE: &str = "view: slices the type does not take";
        /// The check of view that refuses a type the texture is not seen as.
        const VIEW_TYPE_TEXTURE_IS_NOT_SEEN_AS: &str = "view: a type the texture is not seen as";
        /// The check of view that refuses a cube of a texture not square.
        const VIEW_CUBE_OF_TEXTURE_NOT_SQUARE: &str = "view: a cube of a texture not square";
        /// The check of view that refuses a swizzle of a texture drawn into, or a view of a swizzled view.
        const VIEW_SWIZZLE_OF_TEXTURE_DRAWN_INTO_OR_VIEW_OF_SWIZZLED_VIEW: &str = "view: a swizzle of a texture drawn into, or a view of a swizzled view";
        /// The check of sampler that refuses an anisotropy outside one to the most.
        const SAMPLER_ANISOTROPY_OUTSIDE_ONE_TO_MOST: &str = "sampler: an anisotropy outside one to the most";
        /// The check of sampler that refuses an empty level range.
        const SAMPLER_EMPTY_LEVEL_RANGE: &str = "sampler: an empty level range";
        /// The check of sampler that refuses clamps to zero and to the border.
        const SAMPLER_CLAMPS_TO_ZERO_AND_TO_BORDER: &str = "sampler: clamps to zero and to the border";
        /// The check of compute pipeline that refuses a function of another stage.
        const COMPUTE_PIPELINE_FUNCTION_OF_ANOTHER_STAGE: &str = "compute pipeline: a function of another stage";
        /// The check of compute pipeline that refuses a workgroup past the device's most.
        const COMPUTE_PIPELINE_WORKGROUP_PAST_DEVICES_MOST: &str = "compute pipeline: a workgroup past the device's most";
        /// The check of compute pipeline that refuses workgroup memory past the device's.
        const COMPUTE_PIPELINE_WORKGROUP_MEMORY_PAST_DEVICES: &str = "compute pipeline: workgroup memory past the device's";
        /// The check of render that refuses a committed command buffer.
        const RENDER_COMMITTED_COMMAND_BUFFER: &str = "render: a committed command buffer";
        /// The check of blit that refuses a committed command buffer.
        const BLIT_COMMITTED_COMMAND_BUFFER: &str = "blit: a committed command buffer";
        /// The check of compute that refuses a committed command buffer.
        const COMPUTE_COMMITTED_COMMAND_BUFFER: &str = "compute: a committed command buffer";
        /// The check of commit that refuses committed before.
        const COMMIT_COMMITTED_BEFORE: &str = "commit: committed before";
        /// The check of wait that refuses not committed.
        const WAIT_NOT_COMMITTED: &str = "wait: not committed";
        /// The check of layer that refuses off the main thread.
        const LAYER_OFF_MAIN_THREAD: &str = "layer: off the main thread";
        /// The check of layer that refuses a window without an AppKit view.
        const LAYER_WINDOW_WITHOUT_APPKIT_VIEW: &str = "layer: a window without an AppKit view";
        /// The check of layer that refuses a format no layer takes.
        const LAYER_FORMAT_NO_LAYER_TAKES: &str = "layer: a format no layer takes";
        /// The check of layer that refuses an empty size.
        const LAYER_EMPTY_SIZE: &str = "layer: an empty size";
        /// The check of layer that refuses a side past the most.
        const LAYER_SIDE_PAST_MOST: &str = "layer: a side past the most";
        /// The check of drawable that refuses a layer not configured.
        const DRAWABLE_LAYER_NOT_CONFIGURED: &str = "drawable: a layer not configured";
        /// The check of present that refuses a committed command buffer.
        const PRESENT_COMMITTED_COMMAND_BUFFER: &str = "present: a committed command buffer";
        /// The check of present that refuses presented before.
        const PRESENT_PRESENTED_BEFORE: &str = "present: presented before";
        /// The check of timestamps that refuses no places or past the most.
        const TIMESTAMPS_NO_PLACES_OR_PAST_MOST: &str = "timestamps: no places or past the most";
        /// The check of timestamp that refuses a committed command buffer.
        const TIMESTAMP_COMMITTED_COMMAND_BUFFER: &str = "timestamp: a committed command buffer";
        /// The check of timestamp that refuses a place past the last.
        const TIMESTAMP_PLACE_PAST_LAST: &str = "timestamp: a place past the last";
        /// The check of read timestamps that refuses places past the last.
        const READ_TIMESTAMPS_PLACES_PAST_LAST: &str = "read timestamps: places past the last";
        /// The check of read timestamps that refuses uncommitted work writes them.
        const READ_TIMESTAMPS_UNCOMMITTED_WORK_WRITES_THEM: &str = "read timestamps: uncommitted work writes them";
        /// The check of reads that refuses an empty buffer slot.
        const READS_EMPTY_BUFFER_SLOT: &str = "reads: an empty buffer slot";
        /// The check of reads that refuses a buffer shorter than the function reads.
        const READS_BUFFER_SHORTER_THAN_FUNCTION_READS: &str = "reads: a buffer shorter than the function reads";
        /// The check of reads that refuses a buffer offset off the function's alignment.
        const READS_BUFFER_OFFSET_OFF_FUNCTIONS_ALIGNMENT: &str = "reads: a buffer offset off the function's alignment";
        /// The check of reads that refuses bytes at a slot the function writes.
        const READS_BYTES_AT_SLOT_FUNCTION_WRITES: &str = "reads: bytes at a slot the function writes";
        /// The check of reads that refuses immediates shorter than the function's.
        const READS_IMMEDIATES_SHORTER_THAN_FUNCTIONS: &str = "reads: immediates shorter than the function's";
        /// The check of reads that refuses immediates off their alignment.
        const READS_IMMEDIATES_OFF_THEIR_ALIGNMENT: &str = "reads: immediates off their alignment";
        /// The check of reads that refuses an empty texture slot.
        const READS_EMPTY_TEXTURE_SLOT: &str = "reads: an empty texture slot";
        /// The check of reads that refuses a texture not made to be read.
        const READS_TEXTURE_NOT_MADE_TO_BE_READ: &str = "reads: a texture not made to be read";
        /// The check of reads that refuses a texture of another type.
        const READS_TEXTURE_OF_ANOTHER_TYPE: &str = "reads: a texture of another type";
        /// The check of reads that refuses texels of another number type.
        const READS_TEXELS_OF_ANOTHER_NUMBER_TYPE: &str = "reads: texels of another number type";
        /// The check of reads that refuses an empty sampler slot.
        const READS_EMPTY_SAMPLER_SLOT: &str = "reads: an empty sampler slot";
        /// The check of pass that refuses an attachment not made to draw into.
        const PASS_ATTACHMENT_NOT_MADE_TO_DRAW_INTO: &str = "pass: an attachment not made to draw into";
        /// The check of pass that refuses an attachment's level, slice or plane outside it.
        const PASS_ATTACHMENTS_LEVEL_SLICE_OR_PLANE_OUTSIDE_IT: &str = "pass: an attachment's level, slice or plane outside it";
        /// The check of pass that refuses attachments of different sizes or sample counts.
        const PASS_ATTACHMENTS_OF_DIFFERENT_SIZES_OR_SAMPLE_COUNTS: &str = "pass: attachments of different sizes or sample counts";
        /// The check of pass that refuses a depth or stencil texture as a color attachment.
        const PASS_DEPTH_OR_STENCIL_TEXTURE_AS_COLOR_ATTACHMENT: &str = "pass: a depth or stencil texture as a color attachment";
        /// The check of pass that refuses a color attachment of a format no attachment takes.
        const PASS_COLOR_ATTACHMENT_OF_FORMAT_NO_ATTACHMENT_TAKES: &str = "pass: a color attachment of a format no attachment takes";
        /// The check of pass that refuses a depth attachment without depth.
        const PASS_DEPTH_ATTACHMENT_WITHOUT_DEPTH: &str = "pass: a depth attachment without depth";
        /// The check of pass that refuses a depth to clear to outside zero to one.
        const PASS_DEPTH_TO_CLEAR_TO_OUTSIDE_ZERO_TO_ONE: &str = "pass: a depth to clear to outside zero to one";
        /// The check of pass that refuses a stencil attachment without stencil.
        const PASS_STENCIL_ATTACHMENT_WITHOUT_STENCIL: &str = "pass: a stencil attachment without stencil";
        /// The check of pass that refuses depth and stencil of different textures.
        const PASS_DEPTH_AND_STENCIL_OF_DIFFERENT_TEXTURES: &str = "pass: depth and stencil of different textures";
        /// The check of pass that refuses a resolve from or into the wrong sample count.
        const PASS_RESOLVE_FROM_OR_INTO_WRONG_SAMPLE_COUNT: &str = "pass: a resolve from or into the wrong sample count";
        /// The check of pass that refuses a resolve into another format.
        const PASS_RESOLVE_INTO_ANOTHER_FORMAT: &str = "pass: a resolve into another format";
        /// The check of pass that refuses a resolve of a format that does not resolve.
        const PASS_RESOLVE_OF_FORMAT_THAT_DOES_NOT_RESOLVE: &str = "pass: a resolve of a format that does not resolve";
        /// The check of pass that refuses a resolve target's level, slice or plane outside it.
        const PASS_RESOLVE_TARGETS_LEVEL_SLICE_OR_PLANE_OUTSIDE_IT: &str = "pass: a resolve target's level, slice or plane outside it";
        /// The check of pass that refuses a resolve target of another size.
        const PASS_RESOLVE_TARGET_OF_ANOTHER_SIZE: &str = "pass: a resolve target of another size";
        /// The check of pass that refuses a stencil to clear to that a stencil does not hold.
        const PASS_STENCIL_TO_CLEAR_TO_THAT_STENCIL_DOES_NOT_HOLD: &str = "pass: a stencil to clear to that a stencil does not hold";
        /// The check of pass that refuses a visibility result buffer shorter than one result.
        const PASS_VISIBILITY_RESULT_BUFFER_SHORTER_THAN_ONE_RESULT: &str = "pass: a visibility result buffer shorter than one result";
        /// The check of pass that refuses no attachment and an area outside one to the longest side.
        const PASS_NO_ATTACHMENT_AND_AREA_OUTSIDE_ONE_TO_LONGEST_SIDE: &str = "pass: no attachment and an area outside one to the longest side";
        /// The check of pass that refuses two attachments of one plane.
        const PASS_TWO_ATTACHMENTS_OF_ONE_PLANE: &str = "pass: two attachments of one plane";
        /// The check of render that refuses a slot past the last.
        const RENDER_SLOT_PAST_LAST: &str = "render: a slot past the last";
        /// The check of draw that refuses a depth test with no depth attachment.
        const DRAW_DEPTH_TEST_WITH_NO_DEPTH_ATTACHMENT: &str = "draw: a depth test with no depth attachment";
        /// The check of draw that refuses a stencil test with no stencil attachment.
        const DRAW_STENCIL_TEST_WITH_NO_STENCIL_ATTACHMENT: &str = "draw: a stencil test with no stencil attachment";
        /// The check of render that refuses a pipeline made for other attachments.
        const RENDER_PIPELINE_MADE_FOR_OTHER_ATTACHMENTS: &str = "render: a pipeline made for other attachments";
        /// The check of render that refuses a pipeline made for another sample count.
        const RENDER_PIPELINE_MADE_FOR_ANOTHER_SAMPLE_COUNT: &str = "render: a pipeline made for another sample count";
        /// The check of render that refuses a bound range past its buffer.
        const RENDER_BOUND_RANGE_PAST_ITS_BUFFER: &str = "render: a bound range past its buffer";
        /// The check of render that refuses more bytes than Metal copies.
        const RENDER_MORE_BYTES_THAN_METAL_COPIES: &str = "render: more bytes than Metal copies";
        /// The check of render that refuses no viewports or past the most.
        const RENDER_NO_VIEWPORTS_OR_PAST_MOST: &str = "render: no viewports or past the most";
        /// The check of render that refuses a viewport not finite.
        const RENDER_VIEWPORT_NOT_FINITE: &str = "render: a viewport not finite";
        /// The check of render that refuses no scissor rectangles or past the most.
        const RENDER_NO_SCISSOR_RECTANGLES_OR_PAST_MOST: &str = "render: no scissor rectangles or past the most";
        /// The check of render that refuses a scissor rectangle outside the pass.
        const RENDER_SCISSOR_RECTANGLE_OUTSIDE_PASS: &str = "render: a scissor rectangle outside the pass";
        /// The check of render that refuses a visibility offset off its step or past the span.
        const RENDER_VISIBILITY_OFFSET_OFF_ITS_STEP_OR_PAST_SPAN: &str = "render: a visibility offset off its step or past the span";
        /// The check of render that refuses counting with no visibility buffer.
        const RENDER_COUNTING_WITH_NO_VISIBILITY_BUFFER: &str = "render: counting with no visibility buffer";
        /// The check of render that refuses a visibility offset past its buffer.
        const RENDER_VISIBILITY_OFFSET_PAST_ITS_BUFFER: &str = "render: a visibility offset past its buffer";
        /// The check of render that refuses counting twice at one offset.
        const RENDER_COUNTING_TWICE_AT_ONE_OFFSET: &str = "render: counting twice at one offset";
        /// The check of draw that refuses vertices or instances past the largest index.
        const DRAW_VERTICES_OR_INSTANCES_PAST_LARGEST_INDEX: &str = "draw: vertices or instances past the largest index";
        /// The check of draw that refuses no instances.
        const DRAW_NO_INSTANCES: &str = "draw: no instances";
        /// The check of indexed draw that refuses indices outside their buffer or off their size.
        const INDEXED_DRAW_INDICES_OUTSIDE_THEIR_BUFFER_OR_OFF_THEIR_SIZE: &str = "indexed draw: indices outside their buffer or off their size";
        /// The check of indexed draw that refuses instances past the largest index.
        const INDEXED_DRAW_INSTANCES_PAST_LARGEST_INDEX: &str = "indexed draw: instances past the largest index";
        /// The check of indexed draw that refuses no instances or no indices.
        const INDEXED_DRAW_NO_INSTANCES_OR_NO_INDICES: &str = "indexed draw: no instances or no indices";
        /// The check of indirect indexed draw that refuses an index offset outside its buffer or off its size.
        const INDIRECT_INDEXED_DRAW_INDEX_OFFSET_OUTSIDE_ITS_BUFFER_OR_OFF_ITS_SIZE: &str = "indirect indexed draw: an index offset outside its buffer or off its size";
        /// The check of render that refuses depth test bounds Metal lacks, or outside zero to one.
        const RENDER_DEPTH_TEST_BOUNDS_METAL_LACKS_OR_OUTSIDE_ZERO_TO_ONE: &str = "render: depth test bounds Metal lacks, or outside zero to one";
        /// The check of render that refuses depth test bounds with no depth attachment.
        const RENDER_DEPTH_TEST_BOUNDS_WITH_NO_DEPTH_ATTACHMENT: &str = "render: depth test bounds with no depth attachment";
        /// The check of draw that refuses no pipeline.
        const DRAW_NO_PIPELINE: &str = "draw: no pipeline";
        /// The check of draw that refuses primitives of another class than the pipeline's.
        const DRAW_PRIMITIVES_OF_ANOTHER_CLASS_THAN_PIPELINES: &str = "draw: primitives of another class than the pipeline's";
        /// The check of indirect that refuses arguments outside their buffer or off four bytes.
        const INDIRECT_ARGUMENTS_OUTSIDE_THEIR_BUFFER_OR_OFF_FOUR_BYTES: &str = "indirect: arguments outside their buffer or off four bytes";
        /// The check of compute that refuses a slot past the last.
        const COMPUTE_SLOT_PAST_LAST: &str = "compute: a slot past the last";
        /// The check of compute that refuses a bound range past its buffer.
        const COMPUTE_BOUND_RANGE_PAST_ITS_BUFFER: &str = "compute: a bound range past its buffer";
        /// The check of compute that refuses more bytes than Metal copies.
        const COMPUTE_MORE_BYTES_THAN_METAL_COPIES: &str = "compute: more bytes than Metal copies";
        /// The check of dispatch that refuses no threadgroups.
        const DISPATCH_NO_THREADGROUPS: &str = "dispatch: no threadgroups";
        /// The check of dispatch that refuses no pipeline.
        const DISPATCH_NO_PIPELINE: &str = "dispatch: no pipeline";
        /// The check of dispatch that refuses more threads on an axis than a u32 holds.
        const DISPATCH_MORE_THREADS_ON_AXIS_THAN_U32_HOLDS: &str = "dispatch: more threads on an axis than a u32 holds";
        /// The check of indirect dispatch that refuses no pipeline.
        const INDIRECT_DISPATCH_NO_PIPELINE: &str = "indirect dispatch: no pipeline";
        /// The check of region that refuses a level or slice outside the texture.
        const REGION_LEVEL_OR_SLICE_OUTSIDE_TEXTURE: &str = "region: a level or slice outside the texture";
        /// The check of region that refuses outside the level.
        const REGION_OUTSIDE_LEVEL: &str = "region: outside the level";
        /// The check of region that refuses partial blocks.
        const REGION_PARTIAL_BLOCKS: &str = "region: partial blocks";
        /// The check of region that refuses empty.
        const REGION_EMPTY: &str = "region: empty";
        /// The check of copy buffers that refuses no bytes, or off whole words.
        const COPY_BUFFERS_NO_BYTES_OR_OFF_WHOLE_WORDS: &str = "copy buffers: no bytes, or off whole words";
        /// The check of copy buffers that refuses outside a buffer.
        const COPY_BUFFERS_OUTSIDE_BUFFER: &str = "copy buffers: outside a buffer";
        /// The check of copy buffers that refuses overlapping ranges.
        const COPY_BUFFERS_OVERLAPPING_RANGES: &str = "copy buffers: overlapping ranges";
        /// The check of copy buffer and texture that refuses a multisampled or drawable texture.
        const COPY_BUFFER_AND_TEXTURE_MULTISAMPLED_OR_DRAWABLE_TEXTURE: &str = "copy buffer and texture: a multisampled or drawable texture";
        /// The check of copy buffer and texture that refuses an aspect the format lacks.
        const COPY_BUFFER_AND_TEXTURE_ASPECT_FORMAT_LACKS: &str = "copy buffer and texture: an aspect the format lacks";
        /// The check of copy buffer and texture that refuses a row pitch short, long or off a block.
        const COPY_BUFFER_AND_TEXTURE_ROW_PITCH_SHORT_LONG_OR_OFF_BLOCK: &str = "copy buffer and texture: a row pitch short, long or off a block";
        /// The check of copy buffer and texture that refuses an image pitch shorter than an image.
        const COPY_BUFFER_AND_TEXTURE_IMAGE_PITCH_SHORTER_THAN_IMAGE: &str = "copy buffer and texture: an image pitch shorter than an image";
        /// The check of copy buffer and texture that refuses an offset off a block.
        const COPY_BUFFER_AND_TEXTURE_OFFSET_OFF_BLOCK: &str = "copy buffer and texture: an offset off a block";
        /// The check of copy buffer and texture that refuses a length that overflows.
        const COPY_BUFFER_AND_TEXTURE_LENGTH_THAT_OVERFLOWS: &str = "copy buffer and texture: a length that overflows";
        /// The check of copy buffer and texture that refuses outside the buffer.
        const COPY_BUFFER_AND_TEXTURE_OUTSIDE_BUFFER: &str = "copy buffer and texture: outside the buffer";
        /// The check of copy textures that refuses formats or sample counts that differ.
        const COPY_TEXTURES_FORMATS_OR_SAMPLE_COUNTS_THAT_DIFFER: &str = "copy textures: formats or sample counts that differ";
        /// The check of copy textures that refuses a drawable's texture.
        const COPY_TEXTURES_DRAWABLES_TEXTURE: &str = "copy textures: a drawable's texture";
        /// The check of copy textures that refuses overlapping regions.
        const COPY_TEXTURES_OVERLAPPING_REGIONS: &str = "copy textures: overlapping regions";
        /// The check of fill that refuses no bytes, or off whole words.
        const FILL_NO_BYTES_OR_OFF_WHOLE_WORDS: &str = "fill: no bytes, or off whole words";
        /// The check of fill that refuses outside the buffer.
        const FILL_OUTSIDE_BUFFER: &str = "fill: outside the buffer";
        /// The check of mipmaps that refuses of a drawable's texture.
        const MIPMAPS_OF_DRAWABLES_TEXTURE: &str = "mipmaps: of a drawable's texture";
        /// The check of mipmaps that refuses of one level.
        const MIPMAPS_OF_ONE_LEVEL: &str = "mipmaps: of one level";
        /// The check of mipmaps that refuses a format not drawn into and filtered, or multisampled.
        const MIPMAPS_FORMAT_NOT_DRAWN_INTO_AND_FILTERED_OR_MULTISAMPLED: &str = "mipmaps: a format not drawn into and filtered, or multisampled";
        /// The check of function that refuses no such entry point.
        const FUNCTION_NO_SUCH_ENTRY_POINT: &str = "function: no such entry point";
        /// The check of function that refuses a stage other than vertex, fragment and compute.
        const FUNCTION_STAGE_OTHER_THAN_VERTEX_FRAGMENT_AND_COMPUTE: &str = "function: a stage other than vertex, fragment and compute";
        /// The check of function that refuses immediates and lengths not at two slots of their own.
        const FUNCTION_IMMEDIATES_AND_LENGTHS_NOT_AT_TWO_SLOTS_OF_THEIR_OWN: &str = "function: immediates and lengths not at two slots of their own";
        /// The check of function that refuses a global in an address space not checked.
        const FUNCTION_GLOBAL_IN_ADDRESS_SPACE_NOT_CHECKED: &str = "function: a global in an address space not checked";
        /// The check of function that refuses a resource with no binding.
        const FUNCTION_RESOURCE_WITH_NO_BINDING: &str = "function: a resource with no binding";
        /// The check of function that refuses a resource with no slot.
        const FUNCTION_RESOURCE_WITH_NO_SLOT: &str = "function: a resource with no slot";
        /// The check of function that refuses a resource of a type not checked.
        const FUNCTION_RESOURCE_OF_TYPE_NOT_CHECKED: &str = "function: a resource of a type not checked";
        /// The check of function that refuses a buffer slot past the last or taken.
        const FUNCTION_BUFFER_SLOT_PAST_LAST_OR_TAKEN: &str = "function: a buffer slot past the last or taken";
        /// The check of function that refuses an array of bindings.
        const FUNCTION_ARRAY_OF_BINDINGS: &str = "function: an array of bindings";
        /// The check of function that refuses a texture slot past the last.
        const FUNCTION_TEXTURE_SLOT_PAST_LAST: &str = "function: a texture slot past the last";
        /// The check of function that refuses a texture the function writes.
        const FUNCTION_TEXTURE_FUNCTION_WRITES: &str = "function: a texture the function writes";
        /// The check of function that refuses an external texture.
        const FUNCTION_EXTERNAL_TEXTURE: &str = "function: an external texture";
        /// The check of function that refuses a texture type not made.
        const FUNCTION_TEXTURE_TYPE_NOT_MADE: &str = "function: a texture type not made";
        /// The check of function that refuses a sampler slot past the last.
        const FUNCTION_SAMPLER_SLOT_PAST_LAST: &str = "function: a sampler slot past the last";
        /// The check of function that refuses a vertex buffer slot past the last or taken.
        const FUNCTION_VERTEX_BUFFER_SLOT_PAST_LAST_OR_TAKEN: &str = "function: a vertex buffer slot past the last or taken";
        /// The check of function that refuses a vertex stride of zero or past the most.
        const FUNCTION_VERTEX_STRIDE_OF_ZERO_OR_PAST_MOST: &str = "function: a vertex stride of zero or past the most";
        /// The check of function that refuses an attribute past its stride.
        const FUNCTION_ATTRIBUTE_PAST_ITS_STRIDE: &str = "function: an attribute past its stride";
        /// The check of function that refuses texels neither float nor integer.
        const FUNCTION_TEXELS_NEITHER_FLOAT_NOR_INTEGER: &str = "function: texels neither float nor integer";
    }
}

// Metal's unsigned integer is as wide as `u64` on every Mac, so `ns_uinteger` never saturates.
const _: () = assert!(NSUInteger::BITS == u64::BITS);

/// Returns a number as Metal's unsigned integer.
fn ns_uinteger(number: u64) -> NSUInteger {
    NSUInteger::try_from(number).unwrap_or(NSUInteger::MAX)
}

/// Returns the side of a texture's level: the side halved once a level, and never less than one.
fn level_side(side: u32, level: u32) -> u32 {
    side.checked_shr(level).unwrap_or(u32::MIN).max(1)
}

/// Returns a count Metal gives as its unsigned integer, saturated at `u32::MAX`; every count of a texture is far below.
fn count_of(number: NSUInteger) -> u32 {
    u32::try_from(number).unwrap_or(u32::MAX)
}

/// The system's default GPU and one command queue on it.
pub struct DirectMetal {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    /// Bound to vertex buffer slots no command filled, with length zero passed to the function.
    empty: Retained<ProtocolObject<dyn MTLBuffer>>,
    limits: DeviceLimits,
    /// The committed command buffers not yet seen completed, which buffers that they use wait for.
    in_flight: Vec<DirectCommandBuffer>,
    /// The pipeline that checks indirect arguments, made at the first indirect command.
    argument_check: Option<Retained<ProtocolObject<dyn MTLComputePipelineState>>>,
    /// A word only the GPU reaches, which a timestamp's pass fills so that Metal does not skip the pass.
    timestamp_word: Retained<ProtocolObject<dyn MTLBuffer>>,
    /// The event a timestamp's pass waits on, signalled once the work before it ends, and its last value.
    timestamp_event: Retained<ProtocolObject<dyn MTLEvent>>,
    timestamp_value: u64,
    /// Whether encoders take depth test bounds: macOS 26 on.
    depth_test_bounds: bool,
    /// The background builds running.
    background: Arc<AtomicUsize>,
}

/// The sample counts a texture or a pass may have, which the device is asked about.
///
/// https://developer.apple.com/documentation/metal/mtldevice/supportstexturesamplecount(_:)
const SAMPLE_COUNTS: &[u32] = &[1, 2, 4, 8];

/// What the device takes, by its GPU family.
#[derive(Clone, Copy)]
struct DeviceLimits {
    /// The sample counts the device takes, bit n for a count of n.
    sample_counts: u32,
    /// The longest side of a 2D, cube or multisampled texture.
    max_side: u32,
    /// Whether multisampled 32-bit float formats resolve, which GPUs of the Apple9 family on do.
    float32_resolve: bool,
}

/// A buffer of memory the CPU and the GPU share, and the command buffers that use it.
#[derive(Clone)]
pub struct DirectBuffer(Rc<BufferState>);

struct BufferState {
    raw: Retained<ProtocolObject<dyn MTLBuffer>>,
    length: u64,
    /// The command buffers that use it: one dropped uncommitted never runs, and the device keeps a committed one.
    users: RefCell<Vec<Weak<CommandBufferState>>>,
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
    shader_read: bool,
    render_target: bool,
    pixel_format_view: bool,
    /// A drawable's texture, which only passes draw into.
    framebuffer_only: bool,
    /// A view whose reads swizzle its channels, of which no other view is made.
    swizzled: bool,
    /// The texture's storage, the same for every view of it, to tell overlapping copies.
    storage: usize,
    /// The level and slice of the storage that are this view's first, zero for a texture.
    base_level: u32,
    base_slice: u32,
}

/// A sampler state.
#[derive(Clone)]
pub struct DirectSamplerState(Retained<ProtocolObject<dyn MTLSamplerState>>);

/// A depth and stencil state, and the attachments it reads.
#[derive(Clone)]
pub struct DirectDepthStencilState(Rc<DepthStencilState>);

struct DepthStencilState {
    raw: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    /// It tests or writes depth, which needs a depth attachment.
    depth: bool,
    /// It tests stencil, which needs a stencil attachment.
    stencil: bool,
}

/// A compiled entry point and the slots it reads.
#[derive(Clone)]
pub struct DirectFunction(Rc<FunctionState>);

struct FunctionState {
    code: Built<Retained<ProtocolObject<dyn MTLFunction>>>,
    stage: naga::ShaderStage,
    reads: Reads,
}

/// A Metal object made now, or on a thread of its own and taken once that thread is done.
#[derive(Clone)]
enum Built<T> {
    Made(T),
    Building(Slot<T>),
}

/// Where a thread of its own leaves what it built, or why Metal refused it.
type Slot<T> = Arc<(Mutex<Option<Result<T, String>>>, Condvar)>;

impl<T: Clone> Built<T> {
    /// Returns the object, waiting for its thread.
    fn get(&self) -> Result<T, String> {
        match self {
            Built::Made(made) => Ok(made.clone()),
            Built::Building(slot) => {
                let (lock, done) = &**slot;
                let mut held = lock.lock().unwrap_or_else(PoisonError::into_inner);
                loop {
                    if let Some(result) = held.as_ref() {
                        return result.clone();
                    }
                    held = done.wait(held).unwrap_or_else(PoisonError::into_inner);
                }
            }
        }
    }
}

/// Fills a slot when dropped: with what its thread built, or, where the thread ended without, with a refusal.
struct Filling<T>(Slot<T>, Option<Result<T, String>>, Arc<AtomicUsize>);

impl<T> Filling<T> {
    /// Leaves what the thread built in the slot.
    fn finish(mut self, result: Result<T, String>) {
        self.1 = Some(result);
    }
}

impl<T> Drop for Filling<T> {
    fn drop(&mut self) {
        let (lock, done) = &*self.0;
        let result = self
            .1
            .take()
            .unwrap_or_else(|| Err("a background build ended without a result".into()));
        *lock.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
        done.notify_all();
        self.2.fetch_sub(1, Ordering::AcqRel);
    }
}

/// The background builds that may run at once; past them a build is made on the calling thread.
///
/// https://developer.apple.com/documentation/metal/mtldevice/makerenderpipelinestate(descriptor:completionhandler:)
const MOST_BACKGROUND_BUILDS: usize = 8;

/// What a function reads, which a draw or dispatch must have bound.
#[derive(Clone, Default)]
struct Reads {
    /// Buffer slots, the least bytes each must hold past its offset, and what the offset must be a multiple of.
    buffers: Vec<(u32, u64, Alignment)>,
    textures: Vec<(u32, TextureRead)>,
    samplers: Vec<u32>,
    /// The vertex buffer slots it reads its inputs from.
    vertex_buffers: Vec<u32>,
    /// The buffer slots it writes, which must hold buffers rather than bytes.
    written: Vec<u32>,
    /// What each length in the buffer of lengths is the length of, in naga's order.
    sizes: Vec<SizeOf>,
    sizes_index: u32,
    /// The immediates slot, the bytes it must hold, and what their offset must be a multiple of.
    immediates: Option<(u32, u64, Alignment)>,
    workgroup_size: Size,
    /// The bytes of each workgroup variable a compute function reads, in naga's order of its threadgroup arguments.
    threadgroup_lengths: Vec<u64>,
}

/// The texture a function reads at one slot.
#[derive(Clone, Copy, PartialEq, Eq)]
struct TextureRead {
    texture_type: TextureType,
    depth: bool,
    texel: TexelKind,
}

/// The number type texels are read as, which a texture's format and the function's texture type must agree on.
///
/// https://developer.apple.com/documentation/metal/mtlpixelformat
#[derive(Clone, Copy, PartialEq, Eq)]
enum TexelKind {
    Float,
    Sint,
    Uint,
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

/// A render pipeline built on a thread of its own, and what it is made for.
#[derive(Clone)]
pub struct DirectPendingRenderPipelineState(Rc<PendingPipeline>);

struct PendingPipeline {
    raw: Built<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    plan: PipelinePlan,
    vertex: Rc<FunctionState>,
    fragment: Option<Rc<FunctionState>>,
}

/// The plain parts of a render pipeline's descriptor, which a thread of its own builds the pipeline from.
#[derive(Clone, Copy)]
struct PipelinePlan {
    color_attachments: [Option<RenderPipelineColorAttachmentDescriptor>; MAX_COLOR_ATTACHMENTS],
    depth: Option<PixelFormat>,
    stencil: Option<PixelFormat>,
    sample_count: u32,
    alpha_to_coverage: bool,
    rasterization: bool,
    topology: PrimitiveTopologyClass,
}

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

/// Places for timestamps, and how many.
#[derive(Clone)]
pub struct DirectTimestampBuffer(Rc<TimestampBufferState>);

struct TimestampBufferState {
    raw: Retained<ProtocolObject<dyn MTLCounterSampleBuffer>>,
    count: u32,
    /// The command buffers that write into it, which a read waits for.
    users: RefCell<Vec<Weak<CommandBufferState>>>,
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
    DirectPendingRenderPipelineState,
    DirectComputePipelineState,
    DirectCommandBuffer,
    DirectLayer,
    DirectDrawable,
    DirectTimestampBuffer
);

impl DeviceLimits {
    /// Returns whether the device takes a sample count.
    fn takes_samples(&self, count: u32) -> bool {
        1u32.checked_shl(count)
            .is_some_and(|bit| self.sample_counts & bit != 0)
    }
}

impl StandaloneMetal for DirectMetal {
    type Config = ();

    fn new(_config: ()) -> DirectMetal {
        let device = MTLCreateSystemDefaultDevice().expect("a Metal device");
        // The checks follow Apple's feature set tables, which every GPU from the Apple7 family on meets.
        assert!(
            device.supportsFamily(MTLGPUFamily::Apple7),
            "a GPU of the family Apple7 or a later one"
        );
        let sample_counts = SAMPLE_COUNTS
            .iter()
            .filter(|count| device.supportsTextureSampleCount(**count as NSUInteger))
            .fold(u32::MIN, |counts, count| {
                counts | 1u32.checked_shl(*count).unwrap_or(u32::MIN)
            });
        let limits = DeviceLimits {
            sample_counts,
            max_side: if device.supportsFamily(MTLGPUFamily::Apple10) {
                MAX_TEXTURE_SIDE
            } else {
                MAX_SIDE
            },
            float32_resolve: device.supportsFamily(MTLGPUFamily::Apple9),
        };
        let queue = device.newCommandQueue().expect("a command queue");
        let empty = device
            .newBufferWithLength_options(EMPTY_BUFFER_BYTES, MTLResourceOptions::StorageModeShared)
            .expect("a buffer");
        let timestamp_word = device
            .newBufferWithLength_options(
                ns_uinteger(BUFFER_COPY_GRANULE),
                MTLResourceOptions::StorageModePrivate,
            )
            .expect("a buffer");
        let timestamp_event = device.newEvent().expect("an event");
        DirectMetal {
            device,
            queue,
            empty,
            limits,
            depth_test_bounds: objc2::available!(macos = 26.0),
            background: Arc::new(AtomicUsize::new(0)),
            in_flight: Vec::new(),
            argument_check: None,
            timestamp_word,
            timestamp_event,
            timestamp_value: u64::MIN,
        }
    }
}

impl DirectBuffer {
    /// Waits for the committed command buffers that use the buffer; refuses when one is not committed.
    fn settle(&self) -> Result<(), ()> {
        settle_users(&self.0.users)
    }

    /// Notes that a command buffer uses the buffer.
    fn used_by(&self, command_buffer: &DirectCommandBuffer) {
        add_user(&self.0.users, command_buffer);
    }

    /// Returns whether `length` bytes at `offset` lie inside the buffer.
    fn holds(&self, offset: u64, length: u64) -> bool {
        offset
            .checked_add(length)
            .is_some_and(|end| end <= self.0.length)
    }
}

impl DirectMetal {
    /// Checks a render pipeline's descriptor as Metal's validation would, and returns its plain parts.
    fn check_render_pipeline(
        &self,
        descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> Result<PipelinePlan, MakeRenderPipelineStateError> {
        let d = descriptor;
        if refuses!(
            site::PIPELINE_VERTEX_FUNCTION_OF_ANOTHER_STAGE,
            d.vertex_function.0.stage != naga::ShaderStage::Vertex
        ) {
            return Err(MakeRenderPipelineStateError::Invalid(
                "a vertex function of another stage",
            ));
        }
        if refuses!(
            site::PIPELINE_FRAGMENT_FUNCTION_OF_ANOTHER_STAGE,
            d.fragment_function
                .is_some_and(|f| f.0.stage != naga::ShaderStage::Fragment)
        ) {
            return Err(MakeRenderPipelineStateError::Invalid(
                "a fragment function of another stage",
            ));
        }
        if refuses!(
            site::PIPELINE_SAMPLE_COUNT_DEVICE_LACKS,
            !self.limits.takes_samples(d.raster_sample_count)
        ) {
            return Err(MakeRenderPipelineStateError::Invalid(
                "a sample count the device lacks",
            ));
        }
        if refuses!(
            site::PIPELINE_DEPTH_OR_STENCIL_FORMAT_WITHOUT_ASPECT,
            d.depth_attachment_pixel_format
                .is_some_and(|f| !format_facts(f).depth)
                || d.stencil_attachment_pixel_format
                    .is_some_and(|f| !format_facts(f).stencil || f == PixelFormat::X32Stencil8)
        ) {
            return Err(MakeRenderPipelineStateError::Invalid(
                "a depth or stencil format without that aspect",
            ));
        }
        if refuses!(
            site::PIPELINE_COMBINED_DEPTH_AND_STENCIL_FORMAT_BESIDE_ANOTHER,
            matches!(
                (d.depth_attachment_pixel_format, d.stencil_attachment_pixel_format),
                (Some(depth), Some(stencil)) if depth != stencil
                    && (format_facts(depth).stencil || format_facts(stencil).depth)
            )
        ) {
            return Err(MakeRenderPipelineStateError::Invalid(
                "a combined depth and stencil format beside another format",
            ));
        }
        // Metal's validation stops a pipeline that blends with the second source into any attachment but the first, or
        // with more than one color attachment (MEASURED: `OneMinusSource1Alpha` at attachment 3, and at attachment 0
        // beside others).
        let second_source = d.color_attachments.iter().flatten().any(|attachment| {
            attachment.blending.is_some_and(|b| {
                [
                    b.source_rgb_blend_factor,
                    b.destination_rgb_blend_factor,
                    b.source_alpha_blend_factor,
                    b.destination_alpha_blend_factor,
                ]
                .iter()
                .any(|factor| reads_second_source(*factor))
            })
        });
        if refuses!(
            site::PIPELINE_SECOND_SOURCE_BESIDE_OTHER_ATTACHMENTS,
            second_source && d.color_attachments.iter().skip(1).any(Option::is_some)
        ) {
            return Err(MakeRenderPipelineStateError::Invalid(
                "a second source's blend factor in a pipeline of more than its first color attachment",
            ));
        }
        for attachment in d.color_attachments.iter().flatten() {
            let facts = format_facts(attachment.pixel_format);
            if refuses!(
                site::PIPELINE_COLOR_FORMAT_NO_ATTACHMENT_TAKES,
                !facts.color
            ) {
                return Err(MakeRenderPipelineStateError::Invalid(
                    "a color format no color attachment takes",
                ));
            }
            // Metal's validation stops a pipeline that writes some channels of a shared-exponent target, but not all
            // or none (MEASURED: a mask of 0xe into `RGB9E5Float`).
            let mask = attachment.write_mask;
            let channels = [mask.red, mask.green, mask.blue, mask.alpha];
            if refuses!(
                site::PIPELINE_SOME_CHANNELS_OF_SHARED_EXPONENT,
                attachment.pixel_format == PixelFormat::Rgb9e5Float
                    && channels.contains(&true)
                    && channels.contains(&false)
            ) {
                return Err(MakeRenderPipelineStateError::Invalid(
                    "a write mask of some channels of a shared-exponent format",
                ));
            }
            if refuses!(
                site::PIPELINE_BLENDING_FORMAT_THAT_DOES_NOT_BLEND,
                attachment.blending.is_some() && !facts.blend
            ) {
                return Err(MakeRenderPipelineStateError::Invalid(
                    "blending into a format that does not blend",
                ));
            }
        }
        // Metal stops the process when the first color attachment of a pipeline with alpha to coverage is not of a
        // format that blends, a float or normalized one.
        if refuses!(
            site::PIPELINE_ALPHA_TO_COVERAGE_WITHOUT_BLENDING_FIRST_ATTACHMENT,
            d.alpha_to_coverage_enabled
                && !d.color_attachments[0].is_some_and(|a| format_facts(a.pixel_format).blend)
        ) {
            return Err(MakeRenderPipelineStateError::Invalid(
                "alpha to coverage without a first color attachment of a float or normalized format",
            ));
        }
        Ok(PipelinePlan {
            color_attachments: d.color_attachments,
            depth: d.depth_attachment_pixel_format,
            stencil: d.stencil_attachment_pixel_format,
            sample_count: d.raster_sample_count,
            alpha_to_coverage: d.alpha_to_coverage_enabled,
            rasterization: d.rasterization_enabled,
            topology: d.input_primitive_topology,
        })
    }

    /// Runs `build` on a thread of its own and returns where it leaves its result.
    ///
    /// None where the most background builds already run or no thread is made, for the caller to build it itself.
    fn in_background<T: Clone + Send + 'static>(
        &self,
        build: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Option<Built<T>> {
        if self.background.fetch_add(1, Ordering::AcqRel) >= MOST_BACKGROUND_BUILDS {
            self.background.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        let slot: Slot<T> = Arc::new((Mutex::new(None), Condvar::new()));
        let filling = Filling(slot.clone(), None, self.background.clone());
        let spawned = std::thread::Builder::new()
            .name("metal background build".into())
            .spawn(move || filling.finish(build()));
        match spawned {
            Ok(_) => Some(Built::Building(slot)),
            // The closure, and the filling with it, was dropped: the slot holds a refusal, which is not used.
            Err(_) => None,
        }
    }
}

/// Builds a render pipeline of checked plain parts and compiled functions.
fn build_render_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    plan: &PipelinePlan,
    vertex: &ProtocolObject<dyn MTLFunction>,
    fragment: Option<&ProtocolObject<dyn MTLFunction>>,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>, String> {
    let d = plan;
    let raw_descriptor = MTLRenderPipelineDescriptor::new();
    raw_descriptor.setVertexFunction(Some(vertex));
    raw_descriptor.setFragmentFunction(fragment);
    raw_descriptor.setRasterSampleCount(d.sample_count as NSUInteger);
    let topology = match d.topology {
        PrimitiveTopologyClass::Point => MTLPrimitiveTopologyClass::Point,
        PrimitiveTopologyClass::Line => MTLPrimitiveTopologyClass::Line,
        PrimitiveTopologyClass::Triangle => MTLPrimitiveTopologyClass::Triangle,
    };
    #[expect(unsafe_code, reason = "the topology class of a pipeline")]
    // SAFETY: the class is one of Metal's three.
    unsafe {
        raw_descriptor.setInputPrimitiveTopology(topology)
    };
    let attachments = raw_descriptor.colorAttachments();
    for (index, attachment) in d.color_attachments.iter().enumerate() {
        let Some(attachment) = attachment else {
            continue;
        };
        #[expect(unsafe_code, reason = "the read of one color attachment of a pipeline")]
        // SAFETY: the index is below `MAX_COLOR_ATTACHMENTS`, which every Apple GPU has.
        let raw = unsafe { attachments.objectAtIndexedSubscript(index as NSUInteger) };
        raw.setPixelFormat(pixel_format(attachment.pixel_format));
        raw.setWriteMask(write_mask(attachment.write_mask));
        if let Some(blending) = attachment.blending {
            raw.setBlendingEnabled(true);
            raw.setRgbBlendOperation(blend_operation(blending.rgb_blend_operation));
            raw.setAlphaBlendOperation(blend_operation(blending.alpha_blend_operation));
            raw.setSourceRGBBlendFactor(map_blend_factor(blending.source_rgb_blend_factor));
            raw.setDestinationRGBBlendFactor(map_blend_factor(
                blending.destination_rgb_blend_factor,
            ));
            raw.setSourceAlphaBlendFactor(map_blend_factor(blending.source_alpha_blend_factor));
            raw.setDestinationAlphaBlendFactor(map_blend_factor(
                blending.destination_alpha_blend_factor,
            ));
        }
    }
    if let Some(format) = d.depth {
        raw_descriptor.setDepthAttachmentPixelFormat(pixel_format(format));
    }
    if let Some(format) = d.stencil {
        raw_descriptor.setStencilAttachmentPixelFormat(pixel_format(format));
    }
    raw_descriptor.setAlphaToCoverageEnabled(d.alpha_to_coverage);
    raw_descriptor.setRasterizationEnabled(d.rasterization);
    device
        .newRenderPipelineStateWithDescriptor_error(&raw_descriptor)
        .map_err(|error| error.localizedDescription().to_string())
}

/// Writes a function's source as Metal's shading language, and returns it, its entry point's name, and what it reads.
fn function_text(
    source: &FunctionSource<'_>,
) -> Result<(String, String, Reads), MakeFunctionError> {
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
    Ok((text, name, reads))
}

/// Compiles a function's Metal shading language and returns its entry point `name`.
fn compile_function(
    device: &ProtocolObject<dyn MTLDevice>,
    text: &str,
    name: &str,
) -> Result<Retained<ProtocolObject<dyn MTLFunction>>, String> {
    let compile_options = MTLCompileOptions::new();
    compile_options.setLanguageVersion(MTLLanguageVersion::Version3_1);
    let library = device
        .newLibraryWithSource_options_error(&NSString::from_str(text), Some(&compile_options))
        .map_err(|error| error.localizedDescription().to_string())?;
    library
        .newFunctionWithName(&NSString::from_str(name))
        .ok_or_else(|| format!("no function {name}"))
}

impl Metal for DirectMetal {
    type Buffer = DirectBuffer;
    type Texture = DirectTexture;
    type SamplerState = DirectSamplerState;
    type DepthStencilState = DirectDepthStencilState;
    type Function = DirectFunction;
    type RenderPipelineState = DirectRenderPipelineState;
    type ComputePipelineState = DirectComputePipelineState;
    type PendingRenderPipelineState = DirectPendingRenderPipelineState;
    type CommandBuffer = DirectCommandBuffer;
    type Layer = DirectLayer;
    type Drawable = DirectDrawable;
    type TimestampBuffer = DirectTimestampBuffer;

    fn name(&self) -> String {
        self.device.name().to_string()
    }

    fn max_buffer_length(&self) -> u64 {
        (self.device.maxBufferLength() as u64).saturating_sub(u64::from(MAX_VERTEX_STRIDE))
    }

    fn supports_depth_test_bounds(&self) -> bool {
        self.depth_test_bounds
    }

    fn make_buffer(&mut self, length: u64) -> Result<DirectBuffer, MakeBufferError> {
        if refuses!(
            site::BUFFER_LONGER_THAN_DEVICES_MOST,
            length > self.max_buffer_length()
        ) {
            return Err(MakeBufferError::TooLong);
        }
        // Every buffer holds `MAX_VERTEX_STRIDE` unread bytes past its length: Metal's validation stops a draw whose
        // vertex buffer holds less than one element from its offset (MEASURED: 16 bytes at a stride of 24), though the
        // function's bounds checks read none of it. So a buffer of no bytes is one Metal makes too.
        let raw = self
            .device
            .newBufferWithLength_options(
                ns_uinteger(length.saturating_add(u64::from(MAX_VERTEX_STRIDE))),
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
        if refuses!(
            site::WRITE_PAST_BUFFER,
            !buffer.holds(offset, bytes.len() as u64)
        ) {
            return Err(WriteContentsError::OutOfRange);
        }
        if refuses!(
            site::WRITE_BUFFER_UNCOMMITTED_WORK_USES,
            buffer.settle().is_err()
        ) {
            return Err(WriteContentsError::UsedByUncommittedWork);
        }
        // The place in the buffer's memory, by arithmetic that needs no `unsafe` and is checked by `holds` above.
        let start = buffer
            .0
            .raw
            .contents()
            .cast::<u8>()
            .as_ptr()
            .wrapping_add(ns_uinteger(offset));
        #[expect(unsafe_code, reason = "the copy into a buffer's shared memory")]
        // SAFETY: `holds` checked, without overflow, that the range from `start` lies inside the buffer, which is at
        // least as long; no GPU work uses it, since every command buffer that did has completed and none other was
        // encoded since; the CPU touches it only in these copies.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), start, bytes.len())
        };
        Ok(())
    }

    fn read_contents(
        &mut self,
        buffer: &DirectBuffer,
        offset: u64,
        bytes: &mut [u8],
    ) -> Result<(), ReadContentsError> {
        if refuses!(
            site::READ_PAST_BUFFER,
            !buffer.holds(offset, bytes.len() as u64)
        ) {
            return Err(ReadContentsError::OutOfRange);
        }
        if refuses!(
            site::READ_BUFFER_UNCOMMITTED_WORK_USES,
            buffer.settle().is_err()
        ) {
            return Err(ReadContentsError::UsedByUncommittedWork);
        }
        // The place in the buffer's memory, by arithmetic that needs no `unsafe` and is checked by `holds` above.
        let start = buffer
            .0
            .raw
            .contents()
            .cast::<u8>()
            .as_ptr()
            .wrapping_add(ns_uinteger(offset));
        #[expect(unsafe_code, reason = "the copy out of a buffer's shared memory")]
        // SAFETY: as for `write_contents`: the range from `start` lies inside the buffer, and no GPU work uses it.
        unsafe {
            std::ptr::copy_nonoverlapping(start, bytes.as_mut_ptr(), bytes.len())
        };
        Ok(())
    }

    fn make_texture(
        &mut self,
        descriptor: &TextureDescriptor,
    ) -> Result<DirectTexture, MakeTextureError> {
        let d = descriptor;
        let facts = format_facts(d.pixel_format);
        // A cube is one slice of six faces (MEASURED: Metal's validation stops a cube of array length 5).
        let one_slice = matches!(
            d.texture_type,
            TextureType::Type2D
                | TextureType::Type2DMultisample
                | TextureType::TypeCube
                | TextureType::Type3D
        );
        let max_side = if d.texture_type == TextureType::Type3D {
            MAX_3D_SIDE
        } else {
            self.limits.max_side
        };
        // A cube's six faces are six slices to copy, view and draw into, as each cube's are in a cube array.
        let layers = match d.texture_type {
            TextureType::TypeCube | TextureType::TypeCubeArray => {
                d.array_length.checked_mul(CUBE_FACES)
            }
            _ => Some(d.array_length),
        };
        if refuses!(
            site::TEXTURE_SIDE_PAST_MOST,
            d.width > max_side || d.height > max_side || d.depth > max_side
        ) {
            return Err(MakeTextureError::Invalid(
                "a side longer than the device's longest for the type",
            ));
        }
        if refuses!(
            site::TEXTURE_LAYERS_PAST_MOST,
            layers.is_none_or(|layers| layers > MAX_LAYERS)
        ) {
            return Err(MakeTextureError::Invalid(
                "more layers than the device's most",
            ));
        }
        if refuses!(
            site::TEXTURE_STENCIL_VIEW_FORMAT,
            d.pixel_format == PixelFormat::X32Stencil8
        ) {
            return Err(MakeTextureError::Invalid(
                "a stencil view format, which no texture is made in",
            ));
        }
        if refuses!(
            site::TEXTURE_EMPTY_SIZE,
            d.width == 0 || d.height == 0 || d.depth == 0
        ) {
            return Err(MakeTextureError::Invalid("an empty size"));
        }
        if refuses!(
            site::TEXTURE_DEEPER_THAN_ONE_OUTSIDE_3D,
            d.texture_type != TextureType::Type3D && d.depth != 1
        ) {
            return Err(MakeTextureError::Invalid(
                "a depth other than one outside a volume texture",
            ));
        }
        if refuses!(
            site::TEXTURE_ARRAY_LENGTH_TYPE_DOES_NOT_TAKE,
            one_slice && d.array_length != 1 || d.array_length == 0
        ) {
            return Err(MakeTextureError::Invalid(
                "an array length the type does not take",
            ));
        }
        if refuses!(
            site::TEXTURE_CUBE_NOT_SQUARE,
            matches!(
                d.texture_type,
                TextureType::TypeCube | TextureType::TypeCubeArray
            ) && d.width != d.height
        ) {
            return Err(MakeTextureError::Invalid("a cube that is not square"));
        }
        if refuses!(
            site::TEXTURE_LEVELS_PAST_CHAIN,
            d.mipmap_level_count == 0
                || d.mipmap_level_count > full_chain(d.width.max(d.height).max(d.depth))
        ) {
            return Err(MakeTextureError::Invalid("more levels than the size has"));
        }
        if refuses!(
            site::TEXTURE_SAMPLE_COUNT_TYPE_DOES_NOT_TAKE,
            (d.sample_count > 1) != (d.texture_type == TextureType::Type2DMultisample)
                || !self.limits.takes_samples(d.sample_count)
                || d.sample_count > 1 && d.mipmap_level_count != 1
        ) {
            return Err(MakeTextureError::Invalid(
                "a sample count the type does not take",
            ));
        }
        if refuses!(
            site::TEXTURE_COMPRESSED_IN_TYPE_THAT_IS_NOT,
            facts.compressed() && (d.texture_type == TextureType::Type3D || d.sample_count > 1)
        ) {
            return Err(MakeTextureError::Invalid(
                "a compressed format the type does not take",
            ));
        }
        if refuses!(
            site::TEXTURE_DEPTH_OR_STENCIL_IN_3D,
            facts.depth_or_stencil && d.texture_type == TextureType::Type3D
        ) {
            return Err(MakeTextureError::Invalid(
                "a volume texture of depth or stencil",
            ));
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
        // Each size below was checked above against the type, the format and this device's limits from Apple's feature
        // set tables, past which Metal stops the process.
        #[expect(unsafe_code, reason = "the width of a texture to make")]
        // SAFETY: at most the device's longest side for the type, checked above.
        unsafe {
            raw_descriptor.setWidth(d.width as NSUInteger)
        };
        #[expect(unsafe_code, reason = "the height of a texture to make")]
        // SAFETY: at most the device's longest side for the type, checked above.
        unsafe {
            raw_descriptor.setHeight(d.height as NSUInteger)
        };
        #[expect(unsafe_code, reason = "the depth of a texture to make")]
        // SAFETY: one outside a volume texture, and at most the longest volume side in one, checked above.
        unsafe {
            raw_descriptor.setDepth(d.depth as NSUInteger)
        };
        #[expect(unsafe_code, reason = "the level count of a texture to make")]
        // SAFETY: one to the levels of the full chain of the longest side, one if multisampled, checked above.
        unsafe {
            raw_descriptor.setMipmapLevelCount(d.mipmap_level_count as NSUInteger)
        };
        #[expect(unsafe_code, reason = "the sample count of a texture to make")]
        // SAFETY: a count the device takes, above one only for a multisampled type, checked above.
        unsafe {
            raw_descriptor.setSampleCount(d.sample_count as NSUInteger)
        };
        #[expect(unsafe_code, reason = "the array length of a texture to make")]
        // SAFETY: one for a type of one slice, and its layers within the device's most otherwise, checked above.
        unsafe {
            raw_descriptor.setArrayLength(d.array_length as NSUInteger)
        };
        let raw = self
            .device
            .newTextureWithDescriptor(&raw_descriptor)
            .ok_or(MakeTextureError::Refused)?;
        let storage = Retained::as_ptr(&raw).cast::<c_void>() as usize;
        Ok(DirectTexture(Rc::new(TextureState {
            raw,
            texture_type: d.texture_type,
            pixel_format: d.pixel_format,
            width: d.width,
            height: d.height,
            depth: d.depth,
            levels: d.mipmap_level_count,
            slices: layers.unwrap_or(d.array_length),
            sample_count: d.sample_count,
            shader_read: d.usage.shader_read,
            render_target: d.usage.render_target,
            pixel_format_view: d.usage.pixel_format_view,
            framebuffer_only: false,
            swizzled: false,
            storage,
            base_level: u32::MIN,
            base_slice: u32::MIN,
        })))
    }

    fn make_texture_view(
        &mut self,
        texture: &DirectTexture,
        descriptor: &TextureViewDescriptor,
    ) -> Result<DirectTexture, MakeTextureViewError> {
        let t = &texture.0;
        let d = descriptor;
        if refuses!(site::VIEW_OF_DRAWABLES_TEXTURE, t.framebuffer_only) {
            return Err(MakeTextureViewError::Incompatible);
        }
        let levels = d.first_level.checked_add(d.level_count);
        let slices = d.first_slice.checked_add(d.slice_count);
        // The first level and slice of the storage, whose sums with the texture's own are below 2^12 once the view's
        // range is inside the texture, are checked with it.
        let (base_level, base_slice) = (
            t.base_level.checked_add(d.first_level),
            t.base_slice.checked_add(d.first_slice),
        );
        if refuses!(
            site::VIEW_LEVELS_OR_SLICES_PAST_TEXTURE,
            d.level_count == 0
                || d.slice_count == 0
                || levels.is_none_or(|end| end > t.levels)
                || slices.is_none_or(|end| end > t.slices)
                || base_level.is_none()
                || base_slice.is_none()
        ) {
            return Err(MakeTextureViewError::OutOfRange);
        }
        let from = format_facts(t.pixel_format);
        let to = format_facts(d.pixel_format);
        let same_format =
            d.pixel_format == t.pixel_format || from.srgb_twin == Some(d.pixel_format);
        let reinterpreted = t.pixel_format_view
            && !from.depth_or_stencil
            && !to.depth_or_stencil
            && !from.compressed()
            && !to.compressed()
            && from.bytes == to.bytes;
        let stencil_of = t.pixel_format_view
            && t.pixel_format == PixelFormat::Depth32FloatStencil8
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
            TextureType::Type2D => {
                matches!(
                    d.texture_type,
                    TextureType::Type2D | TextureType::Type2DArray
                )
            }
            // Six slices of an array, or of cubes, are seen as a cube (the `square` check below).
            TextureType::Type2DArray | TextureType::TypeCube | TextureType::TypeCubeArray => {
                matches!(
                    d.texture_type,
                    TextureType::Type2D
                        | TextureType::Type2DArray
                        | TextureType::TypeCube
                        | TextureType::TypeCubeArray
                )
            }
        };
        let square = t.width == t.height
            || !matches!(
                d.texture_type,
                TextureType::TypeCube | TextureType::TypeCubeArray
            );
        if refuses!(
            site::VIEW_FORMAT_TEXTURE_IS_NOT_SEEN_AS,
            !(same_format || reinterpreted || stencil_of)
        ) {
            return Err(MakeTextureViewError::Incompatible);
        }
        if refuses!(site::VIEW_SLICES_TYPE_DOES_NOT_TAKE, !slices_fit) {
            return Err(MakeTextureViewError::Incompatible);
        }
        if refuses!(site::VIEW_TYPE_TEXTURE_IS_NOT_SEEN_AS, !type_fits) {
            return Err(MakeTextureViewError::Incompatible);
        }
        if refuses!(site::VIEW_CUBE_OF_TEXTURE_NOT_SQUARE, !square) {
            return Err(MakeTextureViewError::Incompatible);
        }
        let swizzled = d.swizzle != TextureSwizzleChannels::IDENTITY;
        if refuses!(
            site::VIEW_SWIZZLE_OF_TEXTURE_DRAWN_INTO_OR_VIEW_OF_SWIZZLED_VIEW,
            t.swizzled || (swizzled && t.render_target)
        ) {
            return Err(MakeTextureViewError::Incompatible);
        }
        let (base_level, base_slice) = (
            base_level.unwrap_or(u32::MAX),
            base_slice.unwrap_or(u32::MAX),
        );
        let levels = NSRange::new(d.first_level as usize, d.level_count as usize);
        let slices = NSRange::new(d.first_slice as usize, d.slice_count as usize);
        #[expect(unsafe_code, reason = "the call that makes a texture view")]
        // SAFETY: the levels and slices lie inside the texture; the format is the texture's, its sRGB twin, or the
        // stencil or a format of the same size where the texture was made for such views; the type is one Metal makes
        // of the texture's for that many slices; and a swizzle is only on a texture no pass draws into.
        let raw = unsafe {
            t.raw
                .newTextureViewWithPixelFormat_textureType_levels_slices_swizzle(
                    pixel_format(d.pixel_format),
                    texture_type(d.texture_type),
                    levels,
                    slices,
                    swizzle_channels(d.swizzle),
                )
        }
        .ok_or(MakeTextureViewError::Refused)?;
        let size = |n: u32| level_side(n, d.first_level);
        Ok(DirectTexture(Rc::new(TextureState {
            raw,
            texture_type: d.texture_type,
            pixel_format: d.pixel_format,
            width: size(t.width),
            height: size(t.height),
            // Only a 3D texture is deeper than one, and only its levels halve the depth.
            depth: size(t.depth),
            levels: d.level_count,
            slices: d.slice_count,
            sample_count: t.sample_count,
            shader_read: t.shader_read,
            render_target: t.render_target,
            pixel_format_view: false,
            framebuffer_only: false,
            swizzled,
            storage: t.storage,
            base_level,
            base_slice,
        })))
    }

    fn make_sampler_state(
        &mut self,
        descriptor: &SamplerDescriptor,
    ) -> Result<DirectSamplerState, MakeSamplerStateError> {
        let d = descriptor;
        if refuses!(
            site::SAMPLER_ANISOTROPY_OUTSIDE_ONE_TO_MOST,
            d.max_anisotropy == 0 || d.max_anisotropy > MAX_ANISOTROPY
        ) {
            return Err(MakeSamplerStateError::Invalid(
                "an anisotropy outside what a sampler takes",
            ));
        }
        if refuses!(
            site::SAMPLER_EMPTY_LEVEL_RANGE,
            d.lod_min_clamp.is_nan()
                || d.lod_max_clamp.is_nan()
                || d.lod_min_clamp > d.lod_max_clamp
        ) {
            return Err(MakeSamplerStateError::Invalid(
                "a level range that is empty",
            ));
        }
        let modes = [d.s_address_mode, d.t_address_mode, d.r_address_mode];
        if refuses!(
            site::SAMPLER_CLAMPS_TO_ZERO_AND_TO_BORDER,
            modes.contains(&SamplerAddressMode::ClampToZero)
                && modes.contains(&SamplerAddressMode::ClampToBorderColor)
        ) {
            // Metal's validation stops the process: the modes take one of the two, not both.
            return Err(MakeSamplerStateError::Invalid(
                "address modes that clamp both to zero and to the border color",
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
        let raw = self
            .device
            .newDepthStencilStateWithDescriptor(&raw_descriptor)
            .ok_or(MakeDepthStencilStateError::Refused)?;
        Ok(DirectDepthStencilState(Rc::new(DepthStencilState {
            raw,
            depth: descriptor.depth_compare_function != CompareFunction::Always
                || descriptor.depth_write_enabled,
            stencil: descriptor.front_face_stencil.is_some()
                || descriptor.back_face_stencil.is_some(),
        })))
    }

    fn make_function(
        &mut self,
        source: &FunctionSource<'_>,
    ) -> Result<DirectFunction, MakeFunctionError> {
        let (text, name, reads) = function_text(source)?;
        let raw =
            compile_function(&self.device, &text, &name).map_err(MakeFunctionError::Compilation)?;
        Ok(DirectFunction(Rc::new(FunctionState {
            code: Built::Made(raw),
            stage: source.stage,
            reads,
        })))
    }

    fn make_function_in_background(
        &mut self,
        source: &FunctionSource<'_>,
    ) -> Result<DirectFunction, MakeFunctionError> {
        let (text, name, reads) = function_text(source)?;
        let (device, built_text, built_name) = (self.device.clone(), text.clone(), name.clone());
        let code = self.in_background(move || compile_function(&device, &built_text, &built_name));
        let code = match code {
            Some(code) => code,
            None => Built::Made(
                compile_function(&self.device, &text, &name)
                    .map_err(MakeFunctionError::Compilation)?,
            ),
        };
        Ok(DirectFunction(Rc::new(FunctionState {
            code,
            stage: source.stage,
            reads,
        })))
    }

    fn make_render_pipeline_state_in_background(
        &mut self,
        descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> Result<DirectPendingRenderPipelineState, MakeRenderPipelineStateError> {
        let plan = self.check_render_pipeline(descriptor)?;
        let (vertex, fragment) = (
            descriptor.vertex_function.0.clone(),
            descriptor.fragment_function.map(|f| f.0.clone()),
        );
        let (vertex_code, fragment_code) = (
            vertex.code.clone(),
            fragment.as_ref().map(|f| f.code.clone()),
        );
        let device = self.device.clone();
        let build = move || {
            let vertex = vertex_code.get()?;
            let fragment = fragment_code.map(|f| f.get()).transpose()?;
            build_render_pipeline(&device, &plan, &vertex, fragment.as_deref())
        };
        let raw = match self.in_background(build) {
            Some(raw) => raw,
            None => {
                let vertex_raw = vertex
                    .code
                    .get()
                    .map_err(MakeRenderPipelineStateError::Refused)?;
                let fragment_raw = fragment
                    .as_ref()
                    .map(|f| f.code.get())
                    .transpose()
                    .map_err(MakeRenderPipelineStateError::Refused)?;
                Built::Made(
                    build_render_pipeline(
                        &self.device,
                        &plan,
                        &vertex_raw,
                        fragment_raw.as_deref(),
                    )
                    .map_err(MakeRenderPipelineStateError::Refused)?,
                )
            }
        };
        Ok(DirectPendingRenderPipelineState(Rc::new(PendingPipeline {
            raw,
            plan,
            vertex,
            fragment,
        })))
    }

    fn finish_render_pipeline_state(
        &mut self,
        pending: &DirectPendingRenderPipelineState,
    ) -> Result<DirectRenderPipelineState, MakeRenderPipelineStateError> {
        let p = &pending.0;
        let raw = p.raw.get().map_err(MakeRenderPipelineStateError::Refused)?;
        Ok(DirectRenderPipelineState(Rc::new(RenderPipelineState {
            raw,
            vertex: p.vertex.clone(),
            fragment: p.fragment.clone(),
            colors: p.plan.color_attachments.map(|a| a.map(|a| a.pixel_format)),
            depth: p.plan.depth,
            stencil: p.plan.stencil,
            sample_count: p.plan.sample_count,
            topology: p.plan.topology,
        })))
    }

    fn make_render_pipeline_state(
        &mut self,
        descriptor: &RenderPipelineDescriptor<'_, Self>,
    ) -> Result<DirectRenderPipelineState, MakeRenderPipelineStateError> {
        let plan = self.check_render_pipeline(descriptor)?;
        let d = descriptor;
        let vertex = d
            .vertex_function
            .0
            .code
            .get()
            .map_err(MakeRenderPipelineStateError::Refused)?;
        let fragment = d
            .fragment_function
            .map(|f| f.0.code.get())
            .transpose()
            .map_err(MakeRenderPipelineStateError::Refused)?;
        let raw = build_render_pipeline(&self.device, &plan, &vertex, fragment.as_deref())
            .map_err(MakeRenderPipelineStateError::Refused)?;
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
        if refuses!(
            site::COMPUTE_PIPELINE_FUNCTION_OF_ANOTHER_STAGE,
            function.0.stage != naga::ShaderStage::Compute
        ) {
            return Err(MakeComputePipelineStateError::NotCompute);
        }
        // The workgroup is checked against the device before the pipeline is made: naga writes its size as the
        // function's most threads, so Metal makes no pipeline that runs fewer, and refuses one past the device's
        // (MEASURED: "Specified total max threads per threadgroup (2048) exceeds ... (1024)").
        let Size {
            width: x,
            height: y,
            depth: z,
        } = function.0.reads.workgroup_size;
        let most = self.device.maxThreadsPerThreadgroup();
        let threads = u64::from(x)
            .checked_mul(u64::from(y))
            .and_then(|n| n.checked_mul(u64::from(z)));
        if refuses!(
            site::COMPUTE_PIPELINE_WORKGROUP_PAST_DEVICES_MOST,
            threads.is_none_or(|n| n > MAX_THREADS_PER_THREADGROUP)
                || x as usize > most.width
                || y as usize > most.height
                || z as usize > most.depth
        ) {
            return Err(MakeComputePipelineStateError::TooLarge(
                "a workgroup larger than the pipeline runs",
            ));
        }
        let compiled = function
            .0
            .code
            .get()
            .map_err(MakeComputePipelineStateError::Refused)?;
        let raw = self
            .device
            .newComputePipelineStateWithFunction_error(&compiled)
            .map_err(|error| {
                MakeComputePipelineStateError::Refused(error.localizedDescription().to_string())
            })?;
        let threadgroup = function
            .0
            .reads
            .threadgroup_lengths
            .iter()
            .try_fold(u64::MIN, |total, length| total.checked_add(*length));
        if refuses!(
            site::COMPUTE_PIPELINE_WORKGROUP_MEMORY_PAST_DEVICES,
            threadgroup.is_none_or(|n| n > self.device.maxThreadgroupMemoryLength() as u64)
        ) {
            return Err(MakeComputePipelineStateError::TooLarge(
                "workgroup variables larger than the device's threadgroup memory",
            ));
        }
        Ok(DirectComputePipelineState(Rc::new(ComputePipelineState {
            raw,
            function: function.0.clone(),
        })))
    }

    fn make_command_buffer(&mut self) -> Result<DirectCommandBuffer, MakeCommandBufferError> {
        // A pool of its own releases the reference Metal autoreleases, which a thread that never drains one keeps: once
        // all the queue's places are kept so, the next command buffer waits for ever (MEASURED: 64 dropped, uncommitted).
        let raw = autoreleasepool(|_| self.queue.commandBuffer())
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
        if refuses!(
            site::RENDER_COMMITTED_COMMAND_BUFFER,
            command_buffer.0.committed.get()
        ) {
            return Err(EncodeRenderCommandsError::Committed);
        }
        let area = check_pass(pass, self.limits)?;
        check_render_commands(pass, area, commands, self.depth_test_bounds)?;
        let jobs = render_argument_jobs(commands);
        let checked = self
            .encode_argument_checks(command_buffer, &jobs)
            .map_err(EncodeRenderCommandsError::Invalid)?;
        let descriptor = pass_descriptor(pass);
        // The encoder holds its command buffer, so it is released at once as in `make_command_buffer`.
        let encoder = autoreleasepool(|_| {
            command_buffer
                .0
                .raw
                .renderCommandEncoderWithDescriptor(&descriptor)
        })
        .ok_or(EncodeRenderCommandsError::Invalid(
            "Metal made no render encoder",
        ))?;
        let mut used = Vec::new();
        if let Some(buffer) = pass.visibility_result_buffer {
            used.push(buffer.clone());
        }
        let mut bound = RenderBindings::default();
        let mut next = CheckedArguments {
            buffer: checked.as_deref(),
            next: u64::MIN,
        };
        for command in commands {
            encode_render_command(
                &encoder,
                &self.empty,
                &mut bound,
                command,
                &mut used,
                &mut next,
            );
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
        if refuses!(
            site::BLIT_COMMITTED_COMMAND_BUFFER,
            command_buffer.0.committed.get()
        ) {
            return Err(EncodeBlitCommandsError::Committed);
        }
        for command in commands {
            check_blit(command, self.limits)?;
        }
        // Released at once, as in `encode_render_commands`.
        let encoder = autoreleasepool(|_| command_buffer.0.raw.blitCommandEncoder()).ok_or(
            EncodeBlitCommandsError::Invalid("Metal made no blit encoder"),
        )?;
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
        if refuses!(
            site::COMPUTE_COMMITTED_COMMAND_BUFFER,
            command_buffer.0.committed.get()
        ) {
            return Err(EncodeComputeCommandsError::Committed);
        }
        check_compute_commands(commands)?;
        let jobs = compute_argument_jobs(commands);
        let checked = self
            .encode_argument_checks(command_buffer, &jobs)
            .map_err(EncodeComputeCommandsError::Invalid)?;
        let mut next = CheckedArguments {
            buffer: checked.as_deref(),
            next: u64::MIN,
        };
        // Released at once, as in `encode_render_commands`.
        let encoder = autoreleasepool(|_| command_buffer.0.raw.computeCommandEncoder()).ok_or(
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
                    bound.buffers.insert(*index, (*length, *offset));
                    buffer.used_by(command_buffer);
                    // The bound range lies inside the buffer and the slot below `MAX_BUFFER_SLOTS`, both checked by
                    // `check_compute_commands`; the command buffer keeps the buffer.
                    set_compute_buffer(&encoder, &buffer.0.raw, *offset, *index);
                }
                ComputeCommand::SetBytes { bytes, index } => {
                    bound.buffers.insert(*index, (bytes.len() as u64, u64::MIN));
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
                ComputeCommand::DispatchThreadgroups(_)
                | ComputeCommand::DispatchThreadgroupsIndirect { .. } => {
                    let Some(pipeline) = pipeline else {
                        unreachable!("checked by check_compute_commands")
                    };
                    let function = &pipeline.0.function;
                    let sizes = lengths(&function.reads, &bound);
                    if !sizes.is_empty() {
                        set_compute_bytes(&encoder, &sizes, function.reads.sizes_index);
                    }
                    for (index, length) in function.reads.threadgroup_lengths.iter().enumerate() {
                        let length = ns_uinteger(*length);
                        #[expect(
                            unsafe_code,
                            reason = "the length of one workgroup variable's threadgroup memory"
                        )]
                        // SAFETY: the length is the variable's size rounded up to 16 bytes, all of them together within the
                        // device's threadgroup memory (make_compute_pipeline_state), at the index naga gives the variable.
                        unsafe {
                            encoder.setThreadgroupMemoryLength_atIndex(length, index)
                        };
                    }
                    let threads = mtl_size(function.reads.workgroup_size);
                    match command {
                        ComputeCommand::DispatchThreadgroupsIndirect {
                            indirect_buffer, ..
                        } => {
                            indirect_buffer.used_by(command_buffer);
                            let Some((checked, at)) = next.take() else {
                                unreachable!("one checked copy for each indirect command")
                            };
                            #[expect(
                                unsafe_code,
                                reason = "a dispatch of threadgroups the GPU reads"
                            )]
                            // SAFETY: as for a direct dispatch; and the counts are read from direct's own copy, which
                            // `encode_argument_checks` encoded before this pass, each axis within `max_threadgroups` and
                            // its threads within a `u32`, at an offset inside that buffer and a multiple of four.
                            unsafe {
                                encoder
                                    .dispatchThreadgroupsWithIndirectBuffer_indirectBufferOffset_threadsPerThreadgroup(
                                        checked, at, threads,
                                    )
                            };
                        }
                        ComputeCommand::DispatchThreadgroups(groups) => {
                            encoder.dispatchThreadgroups_threadsPerThreadgroup(
                                mtl_size(*groups),
                                threads,
                            );
                        }
                        _ => unreachable!("matched above"),
                    }
                }
            }
        }
        encoder.endEncoding();
        Ok(())
    }

    fn commit(&mut self, command_buffer: &DirectCommandBuffer) -> Result<(), CommitError> {
        if refuses!(
            site::COMMIT_COMMITTED_BEFORE,
            command_buffer.0.committed.replace(true)
        ) {
            return Err(CommitError::AlreadyCommitted);
        }
        command_buffer.0.raw.commit();
        self.in_flight
            .retain(|other| other.0.raw.status() != MTLCommandBufferStatus::Completed);
        self.in_flight.push(command_buffer.clone());
        Ok(())
    }

    fn wait_until_completed(
        &mut self,
        command_buffer: &DirectCommandBuffer,
    ) -> Result<(), WaitUntilCompletedError> {
        if refuses!(site::WAIT_NOT_COMMITTED, !command_buffer.0.committed.get()) {
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
        if refuses!(
            site::LAYER_OFF_MAIN_THREAD,
            MainThreadMarker::new().is_none()
        ) {
            return Err(MakeLayerError::NotMainThread);
        }
        let handle = window.window_handle();
        let appkit = handle
            .as_ref()
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::AppKit(appkit) => Some(appkit),
                _ => None,
            });
        let Some(appkit) = found!(site::LAYER_WINDOW_WITHOUT_APPKIT_VIEW, appkit) else {
            return Err(MakeLayerError::NotAppKit);
        };
        #[expect(unsafe_code, reason = "the layer of the view a window handle names")]
        // SAFETY: the handle borrows its view, so the pointer is a live NSView during the call, which is on the main thread.
        let layer = unsafe { raw_window_metal::Layer::from_ns_view(appkit.ns_view) };
        let pointer = layer.into_raw().cast::<CAMetalLayer>().as_ptr();
        #[expect(
            unsafe_code,
            reason = "the move of the layer's retain into a typed handle"
        )]
        // SAFETY: `into_raw` gave a retained `CAMetalLayer`, whose one retain the handle takes over.
        let raw = unsafe { Retained::from_raw(pointer) }.ok_or(MakeLayerError::NotAppKit)?;
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
        if refuses!(
            site::LAYER_FORMAT_NO_LAYER_TAKES,
            !matches!(
                descriptor.pixel_format,
                PixelFormat::Bgra8Unorm
                    | PixelFormat::Bgra8UnormSrgb
                    | PixelFormat::Rgba16Float
                    | PixelFormat::Rgb10a2Unorm
            )
        ) {
            return Err(ConfigureLayerError::Invalid("a format no layer takes"));
        }
        if refuses!(
            site::LAYER_EMPTY_SIZE,
            descriptor.width == 0 || descriptor.height == 0
        ) {
            return Err(ConfigureLayerError::Invalid("an empty size"));
        }
        if refuses!(
            site::LAYER_SIDE_PAST_MOST,
            descriptor.width > self.limits.max_side || descriptor.height > self.limits.max_side
        ) {
            return Err(ConfigureLayerError::Invalid(
                "a side longer than the device's longest texture side",
            ));
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
        let descriptor = found!(
            site::DRAWABLE_LAYER_NOT_CONFIGURED,
            layer.0.configured.get()
        )
        .ok_or(NextDrawableError::NotConfigured)?;
        let raw =
            autoreleasepool(|_| layer.0.raw.nextDrawable()).ok_or(NextDrawableError::TimedOut)?;
        let texture = raw.texture();
        let storage = Retained::as_ptr(&texture).cast::<c_void>() as usize;
        let state = TextureState {
            raw: texture.clone(),
            width: count_of(texture.width()),
            height: count_of(texture.height()),
            texture_type: TextureType::Type2D,
            pixel_format: descriptor.pixel_format,
            depth: count_of(texture.depth()),
            levels: count_of(texture.mipmapLevelCount()),
            slices: count_of(texture.arrayLength()),
            sample_count: count_of(texture.sampleCount()),
            shader_read: false,
            render_target: true,
            pixel_format_view: false,
            framebuffer_only: true,
            swizzled: false,
            storage,
            base_level: u32::MIN,
            base_slice: u32::MIN,
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
        if refuses!(
            site::PRESENT_COMMITTED_COMMAND_BUFFER,
            command_buffer.0.committed.get()
        ) {
            return Err(PresentDrawableError::Committed);
        }
        if refuses!(
            site::PRESENT_PRESENTED_BEFORE,
            drawable.0.presented.replace(true)
        ) {
            return Err(PresentDrawableError::AlreadyPresented);
        }
        let shown = ProtocolObject::from_retained(drawable.0.raw.clone());
        command_buffer.0.raw.presentDrawable(&shown);
        Ok(())
    }

    fn make_timestamp_buffer(
        &mut self,
        sample_count: u32,
    ) -> Result<DirectTimestampBuffer, MakeTimestampBufferError> {
        if refuses!(
            site::TIMESTAMPS_NO_PLACES_OR_PAST_MOST,
            sample_count == 0 || sample_count > MAX_TIMESTAMPS
        ) {
            return Err(MakeTimestampBufferError::Count);
        }
        if !self
            .device
            .supportsCounterSampling(MTLCounterSamplingPoint::AtStageBoundary)
        {
            return Err(MakeTimestampBufferError::Unsupported);
        }
        let name = NSString::from_str(TIMESTAMP_COUNTER_SET);
        let set: Option<Retained<ProtocolObject<dyn MTLCounterSet>>> = self
            .device
            .counterSets()
            .and_then(|sets| sets.iter().find(|set| *set.name() == *name));
        let set = set.ok_or(MakeTimestampBufferError::Unsupported)?;
        let descriptor = MTLCounterSampleBufferDescriptor::new();
        descriptor.setCounterSet(Some(&set));
        descriptor.setStorageMode(MTLStorageMode::Shared);
        #[expect(unsafe_code, reason = "the place count of a timestamp buffer")]
        // SAFETY: one to `MAX_TIMESTAMPS` places, checked above.
        unsafe {
            descriptor.setSampleCount(sample_count as NSUInteger)
        };
        let raw = self
            .device
            .newCounterSampleBufferWithDescriptor_error(&descriptor)
            .map_err(|_| MakeTimestampBufferError::Unsupported)?;
        Ok(DirectTimestampBuffer(Rc::new(TimestampBufferState {
            raw,
            count: sample_count,
            users: RefCell::new(Vec::new()),
        })))
    }

    fn encode_timestamp(
        &mut self,
        command_buffer: &DirectCommandBuffer,
        timestamps: &DirectTimestampBuffer,
        index: u32,
    ) -> Result<(), EncodeTimestampError> {
        if refuses!(
            site::TIMESTAMP_COMMITTED_COMMAND_BUFFER,
            command_buffer.0.committed.get()
        ) {
            return Err(EncodeTimestampError::Committed);
        }
        if refuses!(site::TIMESTAMP_PLACE_PAST_LAST, index >= timestamps.0.count) {
            return Err(EncodeTimestampError::OutOfRange);
        }
        // The pass starts once all the work encoded before it has ended: Metal runs passes that share no resource side
        // by side (MEASURED: a blit pass's timestamp 8 us after the one before, around 178 ms of compute between).
        let value = self
            .timestamp_value
            .checked_add(1)
            .ok_or(EncodeTimestampError::Refused)?;
        self.timestamp_value = value;
        let raw = &command_buffer.0.raw;
        raw.encodeSignalEvent_value(&self.timestamp_event, value);
        raw.encodeWaitForEvent_value(&self.timestamp_event, value);
        let descriptor = MTLBlitPassDescriptor::new();
        #[expect(
            unsafe_code,
            reason = "the read of a blit pass's first timestamp attachment"
        )]
        // SAFETY: index 0 is below the four attachments every blit pass has.
        let attachment = unsafe {
            descriptor
                .sampleBufferAttachments()
                .objectAtIndexedSubscript(NSUInteger::MIN)
        };
        attachment.setSampleBuffer(Some(&timestamps.0.raw));
        let place = index as NSUInteger;
        #[expect(unsafe_code, reason = "no timestamp where a blit pass begins")]
        // SAFETY: Metal's value for no place.
        unsafe {
            attachment.setStartOfEncoderSampleIndex(MTLCounterDontSample)
        };
        #[expect(
            unsafe_code,
            reason = "the place a blit pass writes its timestamp into"
        )]
        // SAFETY: the place is below the buffer's count, checked above.
        unsafe {
            attachment.setEndOfEncoderSampleIndex(place)
        };
        // Released at once, as in `encode_render_commands`. Metal may write no timestamp of a blit pass that does nothing
        // (wgpu-hal 30.0.2, src/metal/command.rs, `enter_blit`), so the pass fills direct's own word. A compute pass
        // instead, one thread writing the word, failed its command buffer as out of memory beside other compute passes
        // (MEASURED).
        let encoder = autoreleasepool(|_| {
            command_buffer
                .0
                .raw
                .blitCommandEncoderWithDescriptor(&descriptor)
        })
        .ok_or(EncodeTimestampError::Refused)?;
        encoder.fillBuffer_range_value(
            &self.timestamp_word,
            NSRange::new(NSUInteger::MIN, ns_uinteger(BUFFER_COPY_GRANULE)),
            u8::MIN,
        );
        encoder.endEncoding();
        add_user(&timestamps.0.users, command_buffer);
        Ok(())
    }

    fn read_timestamps(
        &mut self,
        timestamps: &DirectTimestampBuffer,
        first: u32,
        into: &mut [u64],
    ) -> Result<(), ReadTimestampsError> {
        let count = u32::try_from(into.len()).ok();
        if refuses!(
            site::READ_TIMESTAMPS_PLACES_PAST_LAST,
            count
                .and_then(|count| first.checked_add(count))
                .is_none_or(|end| end > timestamps.0.count)
        ) {
            return Err(ReadTimestampsError::OutOfRange);
        }
        if refuses!(
            site::READ_TIMESTAMPS_UNCOMMITTED_WORK_WRITES_THEM,
            settle_users(&timestamps.0.users).is_err()
        ) {
            return Err(ReadTimestampsError::UsedByUncommittedWork);
        }
        if into.is_empty() {
            return Ok(());
        }
        let places = NSRange::new(first as NSUInteger, into.len());
        // Metal copies the places' values out on the CPU once the work that wrote them completed (MEASURED: a copy into a
        // buffer by the GPU in the same command buffer reads zeros, and one at a pass's end fails it as out of memory).
        #[expect(unsafe_code, reason = "the copy of timestamps out of their buffer")]
        // SAFETY: the places lie inside the buffer, checked above, and no command buffer that writes them still runs.
        let data = unsafe { timestamps.0.raw.resolveCounterRange(places) }
            .ok_or(ReadTimestampsError::Refused)?;
        let bytes = data.to_vec();
        if bytes.len() != into.len().saturating_mul(size_of::<u64>()) {
            return Err(ReadTimestampsError::Refused);
        }
        for (value, word) in into.iter_mut().zip(bytes.as_chunks::<8>().0) {
            *value = u64::from_le_bytes(*word);
        }
        Ok(())
    }
}

/// Notes that a command buffer uses what `users` lists the command buffers of, dropping those completed.
fn add_user(users: &RefCell<Vec<Weak<CommandBufferState>>>, command_buffer: &DirectCommandBuffer) {
    let mut users = users.borrow_mut();
    users.retain(|user| {
        user.upgrade()
            .is_some_and(|user| user.raw.status() != MTLCommandBufferStatus::Completed)
    });
    let weak = Rc::downgrade(&command_buffer.0);
    if !users.iter().any(|user| user.ptr_eq(&weak)) {
        users.push(weak);
    }
}

/// Waits for the committed command buffers of `users`; refuses when one is not committed.
fn settle_users(users: &RefCell<Vec<Weak<CommandBufferState>>>) -> Result<(), ()> {
    let mut users = users.borrow_mut();
    let live: Vec<Rc<CommandBufferState>> = users.iter().filter_map(Weak::upgrade).collect();
    if live.iter().any(|user| !user.committed.get()) {
        return Err(());
    }
    for user in live {
        user.raw.waitUntilCompleted();
    }
    users.clear();
    Ok(())
}

/// The name of the counter set of the GPU's clock.
///
/// https://developer.apple.com/documentation/metal/mtlcommoncountersettimestamp
const TIMESTAMP_COUNTER_SET: &str = "timestamp";

/// The most bytes one command copies into a command buffer for a slot.
///
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/setvertexbytes(_:length:index:)
const MAX_BYTES: usize = 4096;

/// The most threads of one threadgroup on GPUs of the Apple7 family on.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const MAX_THREADS_PER_THREADGROUP: u64 = 1024;

/// The bytes threadgroup memory lengths are a multiple of.
///
/// https://developer.apple.com/documentation/metal/mtlcomputecommandencoder/setthreadgroupmemorylength(_:index:)
const THREADGROUP_GRANULE: u64 = 16;

/// The faces of one cube.
///
/// https://developer.apple.com/documentation/metal/mtltexturetype/typecube
const CUBE_FACES: u32 = 6;

/// Returns the levels of a full mipmap chain for a largest side.
fn full_chain(side: u32) -> u32 {
    u32::BITS.saturating_sub(side.leading_zeros())
}

/// What one stage has bound, by slot.
#[derive(Clone, Default)]
struct StageBindings {
    /// The bytes the bounds checks see at each filled buffer slot, and the offset they start at.
    buffers: BTreeMap<u32, (u64, u64)>,
    /// The buffer slots filled with bytes, which Metal copies and a function may only read.
    bytes: BTreeSet<u32>,
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
    bound
        .buffers
        .get(&slot)
        .map_or(u64::MIN, |(length, _)| *length)
}

/// Returns the buffer of lengths a function's bounds checks read, as bytes, empty when it reads none.
fn lengths(reads: &Reads, bound: &StageBindings) -> Vec<u8> {
    reads
        .sizes
        .iter()
        .flat_map(|size| {
            let length = match size {
                SizeOf::Slot(slot) => bytes_past(bound, *slot),
                SizeOf::Unused => u64::MIN,
            };
            u32::try_from(length).unwrap_or(u32::MAX).to_le_bytes()
        })
        .collect()
}

/// Checks that every slot a function reads holds a resource of the kind and size it reads.
fn check_reads(reads: &Reads, bound: &StageBindings) -> Result<(), &'static str> {
    for (slot, least, alignment) in &reads.buffers {
        let Some((length, offset)) = found!(site::READS_EMPTY_BUFFER_SLOT, bound.buffers.get(slot))
        else {
            return Err("a buffer slot the function reads is empty");
        };
        if refuses!(
            site::READS_BUFFER_SHORTER_THAN_FUNCTION_READS,
            length < least
        ) {
            return Err("a buffer shorter than what the function reads at its slot");
        }
        // Metal takes no buffer offset that is not a multiple of four, whatever the type.
        if refuses!(
            site::READS_BUFFER_OFFSET_OFF_FUNCTIONS_ALIGNMENT,
            !u32::try_from(*offset).is_ok_and(|offset| {
                alignment.is_aligned(offset) && Alignment::FOUR.is_aligned(offset)
            })
        ) {
            return Err(
                "a buffer at an offset not a multiple of what the function reads at its slot",
            );
        }
    }
    // Metal's validation stops a function that writes a slot holding bytes (MEASURED: `cs_shared` at slot 2).
    if refuses!(
        site::READS_BYTES_AT_SLOT_FUNCTION_WRITES,
        reads.written.iter().any(|slot| bound.bytes.contains(slot))
    ) {
        return Err("bytes at a buffer slot the function writes");
    }
    if refuses!(
        site::READS_IMMEDIATES_SHORTER_THAN_FUNCTIONS,
        reads
            .immediates
            .is_some_and(|(slot, least, _)| bytes_past(bound, slot) < least)
    ) {
        return Err("immediates shorter than the function's");
    }
    // Metal's validation stops a draw whose immediates are at an offset off their type's alignment (MEASURED: a
    // `vec4<f32>` at offset 4, found by the fuzzer).
    if refuses!(
        site::READS_IMMEDIATES_OFF_THEIR_ALIGNMENT,
        reads.immediates.is_some_and(|(slot, _, alignment)| {
            bound.buffers.get(&slot).is_some_and(|(_, offset)| {
                !u32::try_from(*offset).is_ok_and(|offset| {
                    alignment.is_aligned(offset) && Alignment::FOUR.is_aligned(offset)
                })
            })
        })
    ) {
        return Err("immediates at an offset not a multiple of their alignment");
    }
    for (slot, read) in &reads.textures {
        let Some(texture) = found!(site::READS_EMPTY_TEXTURE_SLOT, bound.textures.get(slot)) else {
            return Err("a texture slot the function reads is empty");
        };
        if refuses!(
            site::READS_TEXTURE_NOT_MADE_TO_BE_READ,
            !texture.0.shader_read || texture.0.framebuffer_only
        ) {
            return Err("a texture not made to be read by functions");
        }
        let facts = format_facts(texture.0.pixel_format);
        if refuses!(
            site::READS_TEXTURE_OF_ANOTHER_TYPE,
            texture.0.texture_type != read.texture_type || read.depth && !facts.depth
        ) {
            return Err("a texture of another type than the function reads at its slot");
        }
        // Metal's validation stops the process at a draw reading, say, an integer texture as floats.
        if refuses!(
            site::READS_TEXELS_OF_ANOTHER_NUMBER_TYPE,
            facts.texel != read.texel
        ) {
            return Err(
                "a texture whose texels are of another number type than the function reads",
            );
        }
    }
    if refuses!(
        site::READS_EMPTY_SAMPLER_SLOT,
        reads
            .samplers
            .iter()
            .any(|slot| !bound.samplers.contains_key(slot))
    ) {
        return Err("a sampler slot the function reads is empty");
    }
    Ok(())
}

/// Checks a pass's attachments and returns its area and sample count.
fn check_pass(
    pass: &RenderPassDescriptor<'_, DirectMetal>,
    limits: DeviceLimits,
) -> Result<(u32, u32, u32), EncodeRenderCommandsError> {
    let mut area: Option<(u32, u32, u32)> = None;
    let mut fit = |texture: &DirectTexture, level: u32, slice: u32, depth_plane: u32| {
        let t = &texture.0;
        if refuses!(
            site::PASS_ATTACHMENT_NOT_MADE_TO_DRAW_INTO,
            !t.render_target
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "an attachment not made to draw into",
            ));
        }
        // The level and slice of the storage are checked here too, which `check_attachments_apart` then sums.
        if refuses!(
            site::PASS_ATTACHMENTS_LEVEL_SLICE_OR_PLANE_OUTSIDE_IT,
            level >= t.levels
                || slice >= t.slices
                || depth_plane >= level_side(t.depth, level)
                || t.base_level.checked_add(level).is_none()
                || t.base_slice.checked_add(slice).is_none()
        ) {
            return Err(EncodeRenderCommandsError::OutOfRange(
                "an attachment's level, slice or plane",
            ));
        }
        let size = (
            level_side(t.width, level),
            level_side(t.height, level),
            t.sample_count,
        );
        if refuses!(
            site::PASS_ATTACHMENTS_OF_DIFFERENT_SIZES_OR_SAMPLE_COUNTS,
            area.is_some_and(|seen| seen != size)
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "attachments of different sizes or sample counts",
            ));
        }
        area = Some(size);
        Ok(())
    };
    for attachment in pass.color_attachments.iter().flatten() {
        if refuses!(
            site::PASS_DEPTH_OR_STENCIL_TEXTURE_AS_COLOR_ATTACHMENT,
            format_facts(attachment.texture.0.pixel_format).depth_or_stencil
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "a depth or stencil texture as a color attachment",
            ));
        }
        if refuses!(
            site::PASS_COLOR_ATTACHMENT_OF_FORMAT_NO_ATTACHMENT_TAKES,
            !format_facts(attachment.texture.0.pixel_format).color
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "a color attachment of a format no color attachment takes",
            ));
        }
        fit(
            attachment.texture,
            attachment.level,
            attachment.slice,
            attachment.depth_plane,
        )?;
    }
    if let Some(depth) = &pass.depth_attachment {
        if refuses!(
            site::PASS_DEPTH_ATTACHMENT_WITHOUT_DEPTH,
            !format_facts(depth.texture.0.pixel_format).depth
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "a depth attachment without depth",
            ));
        }
        if refuses!(
            site::PASS_DEPTH_TO_CLEAR_TO_OUTSIDE_ZERO_TO_ONE,
            !(0.0..=1.0).contains(&depth.clear_value)
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "a depth to clear to outside zero to one",
            ));
        }
        fit(depth.texture, depth.level, depth.slice, u32::MIN)?;
    }
    if let Some(stencil) = &pass.stencil_attachment {
        if refuses!(
            site::PASS_STENCIL_ATTACHMENT_WITHOUT_STENCIL,
            !format_facts(stencil.texture.0.pixel_format).stencil
                || stencil.texture.0.pixel_format == PixelFormat::X32Stencil8
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "a stencil attachment without stencil",
            ));
        }
        fit(stencil.texture, stencil.level, stencil.slice, u32::MIN)?;
    }
    if refuses!(
        site::PASS_DEPTH_AND_STENCIL_OF_DIFFERENT_TEXTURES,
        matches!(
            (&pass.depth_attachment, &pass.stencil_attachment),
            (Some(depth), Some(stencil)) if depth.texture.0.storage != stencil.texture.0.storage
        )
    ) {
        return Err(EncodeRenderCommandsError::Invalid(
            "depth and stencil attachments of different textures",
        ));
    }
    for attachment in pass.color_attachments.iter().flatten() {
        if let Some(resolve) = &attachment.resolve {
            let (t, r) = (&attachment.texture.0, &resolve.texture.0);
            if refuses!(
                site::PASS_RESOLVE_FROM_OR_INTO_WRONG_SAMPLE_COUNT,
                t.sample_count == 1 || r.sample_count != 1 || !r.render_target
            ) {
                return Err(EncodeRenderCommandsError::Invalid(
                    "a resolve from or into the wrong sample count",
                ));
            }
            if refuses!(
                site::PASS_RESOLVE_INTO_ANOTHER_FORMAT,
                r.pixel_format != t.pixel_format
            ) {
                return Err(EncodeRenderCommandsError::Invalid(
                    "a resolve into another format",
                ));
            }
            let facts = format_facts(t.pixel_format);
            if refuses!(
                site::PASS_RESOLVE_OF_FORMAT_THAT_DOES_NOT_RESOLVE,
                !facts.resolve || facts.float32 && !limits.float32_resolve
            ) {
                return Err(EncodeRenderCommandsError::Invalid(
                    "a resolve of a format that does not resolve",
                ));
            }
            let level = resolve.level;
            if refuses!(
                site::PASS_RESOLVE_TARGETS_LEVEL_SLICE_OR_PLANE_OUTSIDE_IT,
                level >= r.levels
                    || resolve.slice >= r.slices
                    || resolve.depth_plane >= level_side(r.depth, level)
                    || r.base_level.checked_add(level).is_none()
                    || r.base_slice.checked_add(resolve.slice).is_none()
            ) {
                return Err(EncodeRenderCommandsError::OutOfRange(
                    "a resolve target's level, slice or plane",
                ));
            }
            if refuses!(
                site::PASS_RESOLVE_TARGET_OF_ANOTHER_SIZE,
                (level_side(r.width, level), level_side(r.height, level))
                    != (
                        level_side(t.width, attachment.level),
                        level_side(t.height, attachment.level),
                    )
            ) {
                return Err(EncodeRenderCommandsError::Invalid(
                    "a resolve target of another size",
                ));
            }
        }
    }
    check_attachments_apart(pass)?;
    if refuses!(
        site::PASS_STENCIL_TO_CLEAR_TO_THAT_STENCIL_DOES_NOT_HOLD,
        pass.stencil_attachment
            .as_ref()
            .is_some_and(|stencil| stencil_value(stencil.clear_value).is_none())
    ) {
        return Err(EncodeRenderCommandsError::Invalid(
            "a stencil to clear to that is not a whole number from 0 to 255",
        ));
    }
    if refuses!(
        site::PASS_VISIBILITY_RESULT_BUFFER_SHORTER_THAN_ONE_RESULT,
        pass.visibility_result_buffer
            .is_some_and(|buffer| buffer.0.length < VISIBILITY_RESULT_BYTES)
    ) {
        return Err(EncodeRenderCommandsError::OutOfRange(
            "a visibility result buffer",
        ));
    }
    if let Some(area) = area {
        return Ok(area);
    }
    // An area past the longest side of a target ends the GPU's work in an error (MEASURED: a width of 2^32 - 4).
    if refuses!(
        site::PASS_NO_ATTACHMENT_AND_AREA_OUTSIDE_ONE_TO_LONGEST_SIDE,
        !((1..=limits.max_side).contains(&pass.render_target_width)
            && (1..=limits.max_side).contains(&pass.render_target_height)
            && limits.takes_samples(pass.default_raster_sample_count))
    ) {
        return Err(EncodeRenderCommandsError::Invalid(
            "a pass with no attachment and no area, or one past the longest side",
        ));
    }
    Ok((
        pass.render_target_width,
        pass.render_target_height,
        pass.default_raster_sample_count,
    ))
}

/// Returns a stencil value to clear to as Metal's stencil, if it is a whole number a stencil holds.
fn stencil_value(value: f64) -> Option<u32> {
    (u8::MIN..=u8::MAX)
        .find(|n| f64::from(*n) == value)
        .map(u32::from)
}

/// Checks that no two color attachments or resolve targets of a pass are one plane of one texture's storage.
///
/// Metal's validation stops the process at a pass whose attachments overlap.
fn check_attachments_apart(
    pass: &RenderPassDescriptor<'_, DirectMetal>,
) -> Result<(), EncodeRenderCommandsError> {
    let mut planes = Vec::new();
    // `check_pass` refused every level and slice whose sum with the storage's first overflows, so none saturates.
    let plane = |texture: &DirectTexture, level: u32, slice: u32, depth_plane: u32| {
        let t = &texture.0;
        (
            t.storage,
            t.base_level.saturating_add(level),
            t.base_slice.saturating_add(slice),
            depth_plane,
        )
    };
    for attachment in pass.color_attachments.iter().flatten() {
        planes.push(plane(
            attachment.texture,
            attachment.level,
            attachment.slice,
            attachment.depth_plane,
        ));
        if let Some(resolve) = &attachment.resolve {
            planes.push(plane(
                resolve.texture,
                resolve.level,
                resolve.slice,
                resolve.depth_plane,
            ));
        }
    }
    let shared = planes
        .iter()
        .enumerate()
        .any(|(index, seen)| planes.iter().skip(index).skip(1).any(|other| other == seen));
    if refuses!(site::PASS_TWO_ATTACHMENTS_OF_ONE_PLANE, shared) {
        return Err(EncodeRenderCommandsError::Invalid(
            "two attachments of one plane of one texture",
        ));
    }
    Ok(())
}

/// Checks every command of a pass, as it will be encoded, before any is.
fn check_render_commands(
    pass: &RenderPassDescriptor<'_, DirectMetal>,
    (width, height, samples): (u32, u32, u32),
    commands: &[RenderCommand<DirectMetal>],
    depth_test_bounds: bool,
) -> Result<(), EncodeRenderCommandsError> {
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
    let mut depth_stencil: Option<&DirectDepthStencilState> = None;
    // Metal counts into one offset at most once in a pass.
    let mut counted: Vec<u64> = Vec::new();
    let check_slot = |index: u32, most: u32| {
        if refuses!(site::RENDER_SLOT_PAST_LAST, index >= most) {
            Err(EncodeRenderCommandsError::OutOfRange(
                "a slot past the last",
            ))
        } else {
            Ok(())
        }
    };
    // A draw's depth and stencil state reads the attachments it tests.
    let check_attachments = |state: Option<&DirectDepthStencilState>| {
        if refuses!(
            site::DRAW_DEPTH_TEST_WITH_NO_DEPTH_ATTACHMENT,
            state.is_some_and(|state| state.0.depth) && pass.depth_attachment.is_none()
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "a depth test or write with no depth attachment",
            ));
        }
        if refuses!(
            site::DRAW_STENCIL_TEST_WITH_NO_STENCIL_ATTACHMENT,
            state.is_some_and(|state| state.0.stencil) && pass.stencil_attachment.is_none()
        ) {
            return Err(EncodeRenderCommandsError::Invalid(
                "a stencil test with no stencil attachment",
            ));
        }
        Ok(())
    };
    for command in commands {
        match command {
            RenderCommand::SetDepthStencilState(state) => depth_stencil = Some(state),
            RenderCommand::SetRenderPipelineState(state) => {
                let s = &state.0;
                if refuses!(
                    site::RENDER_PIPELINE_MADE_FOR_OTHER_ATTACHMENTS,
                    s.colors.as_slice() != colors.as_slice()
                        || s.depth != depth
                        || s.stencil != stencil
                ) {
                    return Err(EncodeRenderCommandsError::Invalid(
                        "a pipeline made for other attachments",
                    ));
                }
                if refuses!(
                    site::RENDER_PIPELINE_MADE_FOR_ANOTHER_SAMPLE_COUNT,
                    s.sample_count != samples
                ) {
                    return Err(EncodeRenderCommandsError::Invalid(
                        "a pipeline made for another sample count",
                    ));
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
                if refuses!(
                    site::RENDER_BOUND_RANGE_PAST_ITS_BUFFER,
                    !buffer.holds(*offset, *length)
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "a bound range past its buffer's end",
                    ));
                }
                let stage = if matches!(command, RenderCommand::SetVertexBuffer { .. }) {
                    &mut bound.vertex
                } else {
                    &mut bound.fragment
                };
                stage.buffers.insert(*index, (*length, *offset));
                stage.bytes.remove(index);
            }
            RenderCommand::SetVertexBytes { bytes, index }
            | RenderCommand::SetFragmentBytes { bytes, index } => {
                check_slot(*index, MAX_BUFFER_SLOTS)?;
                if refuses!(
                    site::RENDER_MORE_BYTES_THAN_METAL_COPIES,
                    bytes.len() > MAX_BYTES
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "more bytes than Metal copies into a command buffer",
                    ));
                }
                let stage = if matches!(command, RenderCommand::SetVertexBytes { .. }) {
                    &mut bound.vertex
                } else {
                    &mut bound.fragment
                };
                stage.buffers.insert(*index, (bytes.len() as u64, u64::MIN));
                stage.bytes.insert(*index);
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
                if refuses!(
                    site::RENDER_NO_VIEWPORTS_OR_PAST_MOST,
                    viewports.is_empty() || viewports.len() > MAX_VIEWPORTS
                ) {
                    return Err(EncodeRenderCommandsError::Invalid(
                        "no viewports, or more than a draw has",
                    ));
                }
                let finite = |v: &wcontainers_metal::Viewport| {
                    [v.origin_x, v.origin_y, v.width, v.height, v.znear, v.zfar]
                        .iter()
                        .all(|n| n.is_finite())
                };
                if refuses!(
                    site::RENDER_VIEWPORT_NOT_FINITE,
                    !viewports.iter().all(finite)
                ) {
                    return Err(EncodeRenderCommandsError::Invalid(
                        "a viewport that is not finite",
                    ));
                }
            }
            RenderCommand::SetScissorRects(rects) => {
                if refuses!(
                    site::RENDER_NO_SCISSOR_RECTANGLES_OR_PAST_MOST,
                    rects.is_empty() || rects.len() > MAX_VIEWPORTS
                ) {
                    return Err(EncodeRenderCommandsError::Invalid(
                        "no scissor rectangles, or more than a draw has",
                    ));
                }
                let inside = |r: &wcontainers_metal::ScissorRect| {
                    r.x.checked_add(r.width).is_some_and(|end| end <= width)
                        && r.y.checked_add(r.height).is_some_and(|end| end <= height)
                };
                if refuses!(
                    site::RENDER_SCISSOR_RECTANGLE_OUTSIDE_PASS,
                    !rects.iter().all(inside)
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "a scissor rectangle outside the pass",
                    ));
                }
            }
            RenderCommand::SetVisibilityResultMode { mode, offset } => {
                // Metal's validation checks the offset whatever the mode, and with no buffer too (MEASURED: an
                // offset of 15, and of 2^63, with counting off).
                if refuses!(
                    site::RENDER_VISIBILITY_OFFSET_OFF_ITS_STEP_OR_PAST_SPAN,
                    !offset.is_multiple_of(VISIBILITY_RESULT_BYTES)
                        || offset
                            .checked_add(VISIBILITY_RESULT_BYTES)
                            .is_none_or(|end| end > VISIBILITY_RESULT_SPAN)
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "a visibility result offset",
                    ));
                }
                let Some(buffer) = pass.visibility_result_buffer else {
                    if refuses!(
                        site::RENDER_COUNTING_WITH_NO_VISIBILITY_BUFFER,
                        *mode != VisibilityResultMode::Disabled
                    ) {
                        return Err(EncodeRenderCommandsError::Invalid(
                            "counting with no visibility result buffer",
                        ));
                    }
                    continue;
                };
                if refuses!(
                    site::RENDER_VISIBILITY_OFFSET_PAST_ITS_BUFFER,
                    !buffer.holds(*offset, VISIBILITY_RESULT_BYTES)
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "a visibility result offset",
                    ));
                }
                if *mode != VisibilityResultMode::Disabled {
                    if refuses!(
                        site::RENDER_COUNTING_TWICE_AT_ONE_OFFSET,
                        counted.contains(offset)
                    ) {
                        return Err(EncodeRenderCommandsError::Invalid(
                            "counting twice at one offset in a pass",
                        ));
                    }
                    counted.push(*offset);
                }
            }
            RenderCommand::DrawPrimitives {
                primitive_type,
                vertex_start,
                vertex_count,
                instance_count,
                base_instance,
            } => {
                if refuses!(
                    site::DRAW_VERTICES_OR_INSTANCES_PAST_LARGEST_INDEX,
                    vertex_start.checked_add(*vertex_count).is_none()
                        || base_instance.checked_add(*instance_count).is_none()
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "vertices or instances past the largest index",
                    ));
                }
                if refuses!(site::DRAW_NO_INSTANCES, *instance_count == 0) {
                    return Err(EncodeRenderCommandsError::Invalid("a draw of no instances"));
                }
                check_attachments(depth_stencil)?;
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
                let size = index_bytes(*index_type);
                let indices = u64::from(*index_count).checked_mul(size);
                if refuses!(
                    site::INDEXED_DRAW_INDICES_OUTSIDE_THEIR_BUFFER_OR_OFF_THEIR_SIZE,
                    !index_buffer_offset.is_multiple_of(size)
                        || indices
                            .is_none_or(|bytes| !index_buffer.holds(*index_buffer_offset, bytes))
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "indices outside their buffer, or at an offset not a multiple of their size",
                    ));
                }
                if refuses!(
                    site::INDEXED_DRAW_INSTANCES_PAST_LARGEST_INDEX,
                    base_instance.checked_add(*instance_count).is_none()
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "instances past the largest index",
                    ));
                }
                if refuses!(
                    site::INDEXED_DRAW_NO_INSTANCES_OR_NO_INDICES,
                    *instance_count == 0 || *index_count == 0
                ) {
                    return Err(EncodeRenderCommandsError::Invalid(
                        "a draw of no instances or no indices",
                    ));
                }
                check_attachments(depth_stencil)?;
                check_draw(&bound, *primitive_type).map_err(unbound)?;
            }
            RenderCommand::DrawPrimitivesIndirect {
                primitive_type,
                indirect_buffer,
                indirect_buffer_offset,
            } => {
                check_arguments(
                    indirect_buffer,
                    *indirect_buffer_offset,
                    DRAW_ARGUMENTS_BYTES,
                )
                .map_err(EncodeRenderCommandsError::OutOfRange)?;
                check_attachments(depth_stencil)?;
                check_draw(&bound, *primitive_type).map_err(unbound)?;
            }
            RenderCommand::DrawIndexedPrimitivesIndirect {
                primitive_type,
                index_type,
                index_buffer,
                index_buffer_offset,
                indirect_buffer,
                indirect_buffer_offset,
            } => {
                if refuses!(
                    site::INDIRECT_INDEXED_DRAW_INDEX_OFFSET_OUTSIDE_ITS_BUFFER_OR_OFF_ITS_SIZE,
                    !index_buffer_offset.is_multiple_of(index_bytes(*index_type))
                        || !index_buffer.holds(*index_buffer_offset, u64::MIN)
                ) {
                    return Err(EncodeRenderCommandsError::OutOfRange(
                        "an index offset outside its buffer, or not a multiple of the index size",
                    ));
                }
                check_arguments(
                    indirect_buffer,
                    *indirect_buffer_offset,
                    DRAW_INDEXED_ARGUMENTS_BYTES,
                )
                .map_err(EncodeRenderCommandsError::OutOfRange)?;
                check_attachments(depth_stencil)?;
                check_draw(&bound, *primitive_type).map_err(unbound)?;
            }
            RenderCommand::SetDepthTestBounds {
                min_bound,
                max_bound,
            } => {
                if refuses!(
                    site::RENDER_DEPTH_TEST_BOUNDS_METAL_LACKS_OR_OUTSIDE_ZERO_TO_ONE,
                    !depth_test_bounds
                        || !(0.0..=1.0).contains(min_bound)
                        || !(0.0..=1.0).contains(max_bound)
                        || min_bound > max_bound
                ) {
                    return Err(EncodeRenderCommandsError::Invalid(
                        "depth test bounds this Metal lacks, outside zero to one, or the low above the high",
                    ));
                }
                if refuses!(
                    site::RENDER_DEPTH_TEST_BOUNDS_WITH_NO_DEPTH_ATTACHMENT,
                    pass.depth_attachment.is_none()
                ) {
                    return Err(EncodeRenderCommandsError::Invalid(
                        "depth test bounds in a pass with no depth attachment",
                    ));
                }
            }
            RenderCommand::SetCullMode(_)
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
    let Some(pipeline) = found!(site::DRAW_NO_PIPELINE, bound.pipeline.as_ref()) else {
        return Err("a draw with no pipeline");
    };
    let class = match primitive_type {
        PrimitiveType::Point => PrimitiveTopologyClass::Point,
        PrimitiveType::Line | PrimitiveType::LineStrip => PrimitiveTopologyClass::Line,
        PrimitiveType::Triangle | PrimitiveType::TriangleStrip => PrimitiveTopologyClass::Triangle,
    };
    if refuses!(
        site::DRAW_PRIMITIVES_OF_ANOTHER_CLASS_THAN_PIPELINES,
        class != pipeline.0.topology
    ) {
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
    checked: &mut CheckedArguments<'_>,
) {
    match command {
        RenderCommand::SetRenderPipelineState(state) => {
            encoder.setRenderPipelineState(&state.0.raw);
            bound.pipeline = Some(state.clone());
        }
        RenderCommand::SetDepthStencilState(state) => {
            encoder.setDepthStencilState(Some(&state.0.raw))
        }
        RenderCommand::SetVertexBuffer {
            buffer,
            offset,
            length,
            index,
        } => {
            bound.vertex.buffers.insert(*index, (*length, *offset));
            used.push(buffer.clone());
            let at = ns_uinteger(*offset);
            #[expect(unsafe_code, reason = "the binding of a buffer to a vertex slot")]
            // SAFETY: the bound range lies inside the buffer and the slot below `MAX_BUFFER_SLOTS` (check_render_commands);
            // the command buffer keeps the buffer.
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(
                    Some(&buffer.0.raw),
                    at,
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
            bound.fragment.buffers.insert(*index, (*length, *offset));
            used.push(buffer.clone());
            let at = ns_uinteger(*offset);
            #[expect(unsafe_code, reason = "the binding of a buffer to a fragment slot")]
            // SAFETY: as for the vertex slot.
            unsafe {
                encoder.setFragmentBuffer_offset_atIndex(
                    Some(&buffer.0.raw),
                    at,
                    *index as NSUInteger,
                )
            };
        }
        RenderCommand::SetVertexBytes { bytes, index } => {
            bound
                .vertex
                .buffers
                .insert(*index, (bytes.len() as u64, u64::MIN));
            set_vertex_bytes(encoder, bytes, *index);
        }
        RenderCommand::SetFragmentBytes { bytes, index } => {
            bound
                .fragment
                .buffers
                .insert(*index, (bytes.len() as u64, u64::MIN));
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
        // Taken only from macOS 26 on, within zero to one and in order (check_render_commands).
        RenderCommand::SetDepthTestBounds {
            min_bound,
            max_bound,
        } => encoder.setDepthTestMinBound_maxBound(*min_bound, *max_bound),
        RenderCommand::SetVisibilityResultMode { mode, offset } => encoder
            .setVisibilityResultMode_offset(
                match mode {
                    VisibilityResultMode::Disabled => MTLVisibilityResultMode::Disabled,
                    VisibilityResultMode::Boolean => MTLVisibilityResultMode::Boolean,
                    VisibilityResultMode::Counting => MTLVisibilityResultMode::Counting,
                },
                ns_uinteger(*offset),
            ),
        RenderCommand::DrawPrimitives {
            primitive_type,
            vertex_start,
            vertex_count,
            instance_count,
            base_instance,
        } => {
            prepare_draw(encoder, empty, bound);
            let primitives = primitive_type_of(*primitive_type);
            #[expect(unsafe_code, reason = "a draw without indices")]
            // SAFETY: every slot the pipeline's functions read holds a resource of the kind, number type and size they
            // read, a runtime-sized array at least one element so that naga's bound does not wrap, and they read their
            // vertex inputs, and everything else, bounds-checked against the lengths just bound (check_render_commands,
            // prepare_draw).
            unsafe {
                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                    primitives,
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
            let at = ns_uinteger(*index_buffer_offset);
            let primitives = primitive_type_of(*primitive_type);
            let width = match index_type {
                IndexType::UInt16 => MTLIndexType::UInt16,
                IndexType::UInt32 => MTLIndexType::UInt32,
            };
            #[expect(unsafe_code, reason = "a draw with indices")]
            // SAFETY: as for a draw without indices; and the indices lie inside their buffer at an offset that is a
            // multiple of their size (check_render_commands), while the vertices they name are read bounds-checked.
            unsafe {
                encoder
                    .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount_baseVertex_baseInstance(
                        primitives,
                        *index_count as NSUInteger,
                        width,
                        &index_buffer.0.raw,
                        at,
                        *instance_count as NSUInteger,
                        *base_vertex as isize,
                        *base_instance as NSUInteger,
                    )
            };
        }
        RenderCommand::DrawPrimitivesIndirect {
            primitive_type,
            indirect_buffer,
            ..
        } => {
            prepare_draw(encoder, empty, bound);
            used.push(indirect_buffer.clone());
            let Some((arguments, at)) = checked.take() else {
                unreachable!("one checked copy for each indirect command")
            };
            #[expect(
                unsafe_code,
                reason = "a draw without indices of arguments the GPU reads"
            )]
            // SAFETY: as for a draw without indices, whose vertex inputs are read bounds-checked whatever the counts; and
            // the counts are read from direct's own copy (encode_argument_checks, encoded before this pass), whose first
            // vertex and first instance plus their counts fit a `u32`, at an offset inside it and a multiple of four.
            unsafe {
                encoder.drawPrimitives_indirectBuffer_indirectBufferOffset(
                    primitive_type_of(*primitive_type),
                    arguments,
                    at,
                )
            };
        }
        RenderCommand::DrawIndexedPrimitivesIndirect {
            primitive_type,
            index_type,
            index_buffer,
            index_buffer_offset,
            indirect_buffer,
            ..
        } => {
            prepare_draw(encoder, empty, bound);
            used.push(index_buffer.clone());
            used.push(indirect_buffer.clone());
            let Some((arguments, at)) = checked.take() else {
                unreachable!("one checked copy for each indirect command")
            };
            let width = match index_type {
                IndexType::UInt16 => MTLIndexType::UInt16,
                IndexType::UInt32 => MTLIndexType::UInt32,
            };
            #[expect(unsafe_code, reason = "a draw with indices of arguments the GPU reads")]
            // SAFETY: as for a draw with indices; the counts come from direct's own checked copy
            // (encode_argument_checks), where the first index plus the count lie within the indices past
            // `index_buffer_offset` (check_render_commands), or are zero, at an offset inside the copy and a multiple
            // of four.
            unsafe {
                encoder
                    .drawIndexedPrimitives_indexType_indexBuffer_indexBufferOffset_indirectBuffer_indirectBufferOffset(
                        primitive_type_of(*primitive_type),
                        width,
                        &index_buffer.0.raw,
                        ns_uinteger(*index_buffer_offset),
                        arguments,
                        at,
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
                encoder.setVertexBuffer_offset_atIndex(
                    Some(empty),
                    NSUInteger::MIN,
                    *slot as NSUInteger,
                )
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

/// The bytes bound for an empty byte write, whose bounds checks still see none: a slot left empty stops the process.
///
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/setvertexbytes(_:length:index:)
const PADDING: [u8; 4] = [u8::MIN; 4];

/// Returns the bytes, or the padding for none.
fn bytes_or_padding(bytes: &[u8]) -> &[u8] {
    if bytes.is_empty() { &PADDING } else { bytes }
}

/// Copies bytes into the command buffer for a vertex slot.
fn set_vertex_bytes(
    encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    bytes: &[u8],
    index: u32,
) {
    // Bytes shorter than the longest stride are copied with zeros after them, unread: Metal's validation stops a draw
    // whose vertex buffer holds less than one element (MEASURED: 4 bytes at a stride of 24), as for `make_buffer`.
    let mut padded = bytes.to_vec();
    padded.resize(padded.len().max(EMPTY_BUFFER_BYTES), u8::MIN);
    let bytes = padded.as_slice();
    let Some(first) = NonNull::new(bytes.as_ptr().cast_mut().cast::<c_void>()) else {
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
    let bytes = bytes_or_padding(bytes);
    let Some(first) = NonNull::new(bytes.as_ptr().cast_mut().cast::<c_void>()) else {
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
    let bytes = bytes_or_padding(bytes);
    let Some(first) = NonNull::new(bytes.as_ptr().cast_mut().cast::<c_void>()) else {
        return;
    };
    #[expect(unsafe_code, reason = "the copy of bytes into a compute slot")]
    // SAFETY: the bytes are read during the call, and the slot is below `MAX_BUFFER_SLOTS`.
    unsafe {
        encoder.setBytes_length_atIndex(first, bytes.len() as NSUInteger, index as NSUInteger)
    };
}

/// Binds a range of a buffer from `offset` to a compute slot.
fn set_compute_buffer(
    encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
    buffer: &ProtocolObject<dyn MTLBuffer>,
    offset: u64,
    index: u32,
) {
    #[expect(unsafe_code, reason = "the binding of a buffer to a compute slot")]
    // SAFETY: every caller has checked that the offset lies inside the buffer, a multiple of four, and the slot below
    // `MAX_BUFFER_SLOTS`; the command buffer keeps the buffer.
    unsafe {
        encoder.setBuffer_offset_atIndex(Some(buffer), ns_uinteger(offset), index as NSUInteger)
    };
}

/// Returns the bytes of one index of a type.
fn index_bytes(index_type: IndexType) -> u64 {
    match index_type {
        IndexType::UInt16 => u64::from(bytes_of::<u16>()),
        IndexType::UInt32 => u64::from(bytes_of::<u32>()),
    }
}

/// Checks that `bytes` of indirect arguments lie inside their buffer at an offset that is a multiple of four.
fn check_arguments(buffer: &DirectBuffer, offset: u64, bytes: u64) -> Result<(), &'static str> {
    if refuses!(
        site::INDIRECT_ARGUMENTS_OUTSIDE_THEIR_BUFFER_OR_OFF_FOUR_BYTES,
        !offset.is_multiple_of(INDIRECT_ARGUMENTS_ALIGNMENT) || !buffer.holds(offset, bytes)
    ) {
        return Err(
            "indirect arguments outside their buffer, or at an offset not a multiple of four",
        );
    }
    Ok(())
}

/// The function that copies each indirect command's arguments, checked, into direct's own buffer.
///
/// A command reaching past what it is checked against is made one that does nothing. Its kinds are
/// `ARGUMENTS_DRAW`, `ARGUMENTS_DRAW_INDEXED` and `ARGUMENTS_DISPATCH`.
///
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const ARGUMENT_CHECK_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;
struct Job { uint kind; uint index_limit; uint threads[3]; uint max_groups; };
kernel void check_arguments(volatile device const uint* arguments [[buffer(0)]],
                            device uint* out [[buffer(1)]],
                            constant Job& job [[buffer(2)]]) {
    // Every argument is read once, through a volatile pointer, so that the value checked is the value written,
    // whatever else changes the arguments meanwhile.
    const ulong most = 0xffffffffUL;
    if (job.kind == 2u) {
        uint g0 = arguments[0];
        uint g1 = arguments[1];
        uint g2 = arguments[2];
        bool fits = g0 <= job.max_groups && g1 <= job.max_groups && g2 <= job.max_groups
            && ulong(g0) * ulong(job.threads[0]) <= most
            && ulong(g1) * ulong(job.threads[1]) <= most
            && ulong(g2) * ulong(job.threads[2]) <= most;
        out[0] = fits ? g0 : 0u;
        out[1] = fits ? g1 : 0u;
        out[2] = fits ? g2 : 0u;
        return;
    }
    uint count = arguments[0];
    uint instances = arguments[1];
    uint first = arguments[2];
    uint fourth = arguments[3];
    if (job.kind == 0u) {
        bool fits = ulong(first) + ulong(count) <= most && ulong(fourth) + ulong(instances) <= most;
        out[0] = fits ? count : 0u;
        out[1] = fits ? instances : 0u;
        out[2] = fits ? first : 0u;
        out[3] = fits ? fourth : 0u;
        return;
    }
    uint first_instance = arguments[4];
    bool fits = ulong(first) + ulong(count) <= ulong(job.index_limit)
        && ulong(first_instance) + ulong(instances) <= most;
    out[0] = fits ? count : 0u;
    out[1] = fits ? instances : 0u;
    out[2] = fits ? first : 0u;
    out[3] = fits ? fourth : 0u;
    out[4] = fits ? first_instance : 0u;
}
"#;

/// The kind of a draw's arguments in `ARGUMENT_CHECK_SOURCE`.
///
/// https://developer.apple.com/documentation/metal/mtldrawprimitivesindirectarguments
const ARGUMENTS_DRAW: u32 = 0;
/// The kind of an indexed draw's arguments in `ARGUMENT_CHECK_SOURCE`.
///
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const ARGUMENTS_DRAW_INDEXED: u32 = 1;
/// The kind of a dispatch's arguments in `ARGUMENT_CHECK_SOURCE`.
///
/// https://developer.apple.com/documentation/metal/mtldispatchthreadgroupsindirectarguments
const ARGUMENTS_DISPATCH: u32 = 2;

/// The bytes between two commands' checked arguments in direct's buffer: the longest, rounded up to 16.
///
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const CHECKED_ARGUMENTS_STRIDE: u64 = 32;

const _: () = assert!(
    DRAW_INDEXED_ARGUMENTS_BYTES <= CHECKED_ARGUMENTS_STRIDE
        && DRAW_ARGUMENTS_BYTES <= CHECKED_ARGUMENTS_STRIDE
        && DISPATCH_ARGUMENTS_BYTES <= CHECKED_ARGUMENTS_STRIDE
        && CHECKED_ARGUMENTS_STRIDE.is_multiple_of(INDIRECT_ARGUMENTS_ALIGNMENT)
);

/// One indirect command's arguments, and what the check copies them against.
struct ArgumentJob<'a> {
    arguments: &'a DirectBuffer,
    offset: u64,
    kind: u32,
    /// The indices the index buffer holds past its offset, for an indexed draw.
    index_limit: u32,
    /// The threads of a threadgroup on each axis, for a dispatch.
    threads: [u32; 3],
    max_groups: u32,
}

impl ArgumentJob<'_> {
    /// Returns the job as the function's `Job` reads it: six `u32`s.
    fn bytes(&self) -> Vec<u8> {
        [
            self.kind,
            self.index_limit,
            self.threads[0],
            self.threads[1],
            self.threads[2],
            self.max_groups,
        ]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect()
    }
}

/// Where the next indirect command of a pass reads its checked arguments.
struct CheckedArguments<'a> {
    buffer: Option<&'a ProtocolObject<dyn MTLBuffer>>,
    next: u64,
}

impl<'a> CheckedArguments<'a> {
    /// Returns the buffer and offset of the next command's checked arguments, and moves past them.
    fn take(&mut self) -> Option<(&'a ProtocolObject<dyn MTLBuffer>, NSUInteger)> {
        let buffer = self.buffer?;
        let at = self.next.checked_mul(CHECKED_ARGUMENTS_STRIDE)?;
        self.next = self.next.checked_add(1)?;
        Some((buffer, ns_uinteger(at)))
    }
}

/// Returns the argument checks of a checked pass's indirect draws, in order.
fn render_argument_jobs(commands: &[RenderCommand<DirectMetal>]) -> Vec<ArgumentJob<'_>> {
    commands
        .iter()
        .filter_map(|command| match command {
            RenderCommand::DrawPrimitivesIndirect {
                indirect_buffer,
                indirect_buffer_offset,
                ..
            } => Some(ArgumentJob {
                arguments: indirect_buffer,
                offset: *indirect_buffer_offset,
                kind: ARGUMENTS_DRAW,
                index_limit: u32::MIN,
                threads: [u32::MIN; 3],
                max_groups: u32::MIN,
            }),
            RenderCommand::DrawIndexedPrimitivesIndirect {
                index_type,
                index_buffer,
                index_buffer_offset,
                indirect_buffer,
                indirect_buffer_offset,
                ..
            } => {
                // The offset lies inside the buffer (check_render_commands); past `u32::MAX` indices every first index
                // and count a `u32` holds fit, so the limit saturates there.
                let indices = index_buffer
                    .0
                    .length
                    .checked_sub(*index_buffer_offset)
                    .and_then(|bytes| bytes.checked_div(index_bytes(*index_type)))
                    .unwrap_or(u64::MIN);
                Some(ArgumentJob {
                    arguments: indirect_buffer,
                    offset: *indirect_buffer_offset,
                    kind: ARGUMENTS_DRAW_INDEXED,
                    index_limit: u32::try_from(indices).unwrap_or(u32::MAX),
                    threads: [u32::MIN; 3],
                    max_groups: u32::MIN,
                })
            }
            _ => None,
        })
        .collect()
}

/// Returns the argument checks of checked compute commands' indirect dispatches, in order.
fn compute_argument_jobs(commands: &[ComputeCommand<DirectMetal>]) -> Vec<ArgumentJob<'_>> {
    let mut pipeline: Option<&DirectComputePipelineState> = None;
    let mut jobs = Vec::new();
    for command in commands {
        match command {
            ComputeCommand::SetComputePipelineState(state) => pipeline = Some(state),
            ComputeCommand::DispatchThreadgroupsIndirect {
                indirect_buffer,
                indirect_buffer_offset,
                max_threadgroups,
            } => {
                let Some(pipeline) = pipeline else {
                    unreachable!("checked by check_compute_commands")
                };
                let threads = pipeline.0.function.reads.workgroup_size;
                jobs.push(ArgumentJob {
                    arguments: indirect_buffer,
                    offset: *indirect_buffer_offset,
                    kind: ARGUMENTS_DISPATCH,
                    index_limit: u32::MIN,
                    threads: [threads.width, threads.height, threads.depth],
                    max_groups: *max_threadgroups,
                });
            }
            _ => {}
        }
    }
    jobs
}

impl DirectMetal {
    /// Returns the pipeline that checks indirect arguments, compiled at its first use.
    fn argument_check(
        &mut self,
    ) -> Result<Retained<ProtocolObject<dyn MTLComputePipelineState>>, &'static str> {
        if let Some(pipeline) = &self.argument_check {
            return Ok(pipeline.clone());
        }
        let options = MTLCompileOptions::new();
        options.setLanguageVersion(MTLLanguageVersion::Version3_1);
        let library = self
            .device
            .newLibraryWithSource_options_error(
                &NSString::from_str(ARGUMENT_CHECK_SOURCE),
                Some(&options),
            )
            .map_err(|_| "Metal compiled no argument check")?;
        let function = library
            .newFunctionWithName(&NSString::from_str("check_arguments"))
            .ok_or("Metal compiled no argument check")?;
        let pipeline = self
            .device
            .newComputePipelineStateWithFunction_error(&function)
            .map_err(|_| "Metal made no argument check pipeline")?;
        self.argument_check = Some(pipeline.clone());
        Ok(pipeline)
    }

    /// Encodes the checked copy of each job's arguments into a new buffer only the GPU reaches, and returns it.
    ///
    /// In a compute pass of its own, `CHECKED_ARGUMENTS_STRIDE` bytes a job in order; none for no jobs.
    fn encode_argument_checks(
        &mut self,
        command_buffer: &DirectCommandBuffer,
        jobs: &[ArgumentJob<'_>],
    ) -> Result<Option<Retained<ProtocolObject<dyn MTLBuffer>>>, &'static str> {
        if jobs.is_empty() {
            return Ok(None);
        }
        let pipeline = self.argument_check()?;
        let length = u64::try_from(jobs.len())
            .ok()
            .and_then(|n| n.checked_mul(CHECKED_ARGUMENTS_STRIDE))
            .ok_or("more indirect commands than a buffer holds")?;
        let checked = self
            .device
            .newBufferWithLength_options(
                ns_uinteger(length),
                MTLResourceOptions::StorageModePrivate,
            )
            .ok_or("Metal made no buffer for checked arguments")?;
        // Released at once, as in `encode_render_commands`.
        let encoder = autoreleasepool(|_| command_buffer.0.raw.computeCommandEncoder())
            .ok_or("Metal made no compute encoder")?;
        encoder.setComputePipelineState(&pipeline);
        let one = MTLSize {
            width: 1,
            height: 1,
            depth: 1,
        };
        let mut into = u64::MIN;
        for job in jobs {
            job.arguments.used_by(command_buffer);
            // The arguments lie inside their buffer at a multiple of four (check_arguments), as many bytes as the
            // function reads for the job's kind; the copy's place lies inside `checked`, which holds a stride a job.
            set_compute_buffer(&encoder, &job.arguments.0.raw, job.offset, 0);
            set_compute_buffer(&encoder, &checked, into, 1);
            set_compute_bytes(&encoder, &job.bytes(), 2);
            encoder.dispatchThreadgroups_threadsPerThreadgroup(one, one);
            into = into
                .checked_add(CHECKED_ARGUMENTS_STRIDE)
                .ok_or("more indirect commands than a buffer holds")?;
        }
        encoder.endEncoding();
        Ok(Some(checked))
    }
}

/// Checks every compute command before any is encoded.
fn check_compute_commands(
    commands: &[ComputeCommand<DirectMetal>],
) -> Result<(), EncodeComputeCommandsError> {
    let mut bound = StageBindings::default();
    let mut pipeline: Option<&DirectComputePipelineState> = None;
    let check_slot = |index: u32, most: u32| {
        if refuses!(site::COMPUTE_SLOT_PAST_LAST, index >= most) {
            Err(EncodeComputeCommandsError::OutOfRange(
                "a slot past the last",
            ))
        } else {
            Ok(())
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
                if refuses!(
                    site::COMPUTE_BOUND_RANGE_PAST_ITS_BUFFER,
                    !buffer.holds(*offset, *length)
                ) {
                    return Err(EncodeComputeCommandsError::OutOfRange(
                        "a bound range past its buffer's end",
                    ));
                }
                bound.buffers.insert(*index, (*length, *offset));
                bound.bytes.remove(index);
            }
            ComputeCommand::SetBytes { bytes, index } => {
                check_slot(*index, MAX_BUFFER_SLOTS)?;
                if refuses!(
                    site::COMPUTE_MORE_BYTES_THAN_METAL_COPIES,
                    bytes.len() > MAX_BYTES
                ) {
                    return Err(EncodeComputeCommandsError::OutOfRange(
                        "more bytes than Metal copies into a command buffer",
                    ));
                }
                bound.buffers.insert(*index, (bytes.len() as u64, u64::MIN));
                bound.bytes.insert(*index);
            }
            ComputeCommand::SetTexture { texture, index } => {
                check_slot(*index, MAX_TEXTURE_SLOTS)?;
                bound.textures.insert(*index, texture.clone());
            }
            ComputeCommand::SetSamplerState { index, .. } => {
                check_slot(*index, MAX_SAMPLER_SLOTS)?;
                bound.samplers.insert(*index, ());
            }
            ComputeCommand::DispatchThreadgroups(groups) => {
                if refuses!(
                    site::DISPATCH_NO_THREADGROUPS,
                    groups.width == 0 || groups.height == 0 || groups.depth == 0
                ) {
                    return Err(EncodeComputeCommandsError::Invalid(
                        "a dispatch of no threadgroups",
                    ));
                }
                let Some(pipeline) = found!(site::DISPATCH_NO_PIPELINE, pipeline) else {
                    return Err(EncodeComputeCommandsError::Unbound(
                        "a dispatch with no pipeline",
                    ));
                };
                let threads = pipeline.0.function.reads.workgroup_size;
                let axes = [
                    (groups.width, threads.width),
                    (groups.height, threads.height),
                    (groups.depth, threads.depth),
                ];
                // Metal's validation stops a grid whose threads on one axis pass the largest `u32`, which a thread's
                // position in the grid holds (MEASURED: 2^26 groups of 64 threads).
                if refuses!(
                    site::DISPATCH_MORE_THREADS_ON_AXIS_THAN_U32_HOLDS,
                    axes.iter().any(|(groups, threads)| {
                        u32::try_from(u64::from(*groups).saturating_mul(u64::from(*threads)))
                            .is_err()
                    })
                ) {
                    return Err(EncodeComputeCommandsError::OutOfRange(
                        "a grid of more threads on one axis than the largest u32",
                    ));
                }
                check_reads(&pipeline.0.function.reads, &bound)
                    .map_err(EncodeComputeCommandsError::Unbound)?;
            }
            ComputeCommand::DispatchThreadgroupsIndirect {
                indirect_buffer,
                indirect_buffer_offset,
                ..
            } => {
                check_arguments(
                    indirect_buffer,
                    *indirect_buffer_offset,
                    DISPATCH_ARGUMENTS_BYTES,
                )
                .map_err(EncodeComputeCommandsError::OutOfRange)?;
                let Some(pipeline) = found!(site::INDIRECT_DISPATCH_NO_PIPELINE, pipeline) else {
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
        BlitCommand::GenerateMipmaps { .. } => Vec::new(),
    }
}

/// Returns the bytes and the block of what a copy between a buffer and a texture moves, by its option.
fn moved(format: PixelFormat, option: BlitOption) -> Option<(u64, (u32, u32))> {
    let facts = format_facts(format);
    match (option, format) {
        (BlitOption::None, PixelFormat::Depth32FloatStencil8) => None,
        (BlitOption::None, _) => Some((u64::from(facts.bytes), facts.block)),
        (BlitOption::DepthFromDepthStencil, PixelFormat::Depth32FloatStencil8) => {
            Some((u64::from(bytes_of::<f32>()), TEXEL_BLOCK))
        }
        (BlitOption::StencilFromDepthStencil, PixelFormat::Depth32FloatStencil8) => {
            Some((u64::from(bytes_of::<u8>()), TEXEL_BLOCK))
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
    if refuses!(
        site::REGION_LEVEL_OR_SLICE_OUTSIDE_TEXTURE,
        level >= t.levels || slice >= t.slices
    ) {
        return Err("a level or slice outside the texture");
    }
    // Only a 3D texture is deeper than one.
    let (w, h, d) = (
        level_side(t.width, level),
        level_side(t.height, level),
        level_side(t.depth, level),
    );
    let fits = |at: u32, n: u32, most: Option<u32>| {
        at.checked_add(n)
            .is_some_and(|end| most.is_some_and(|most| end <= most))
    };
    // A region may end at the level's edge inside a block, which counts as whole; Metal's validation stops one that
    // reaches past the level's own texels into that block (MEASURED: 4 rows into a level of 1 of ASTC 4x4).
    let (bw, bh) = block;
    if refuses!(
        site::REGION_OUTSIDE_LEVEL,
        !fits(origin.x, size.width, Some(w))
            || !fits(origin.y, size.height, Some(h))
            || !fits(origin.z, size.depth, Some(d))
    ) {
        return Err("a region outside the level");
    }
    let edge = |at: u32, n: u32, most: u32, b: u32| {
        at.is_multiple_of(b) && (n.is_multiple_of(b) || at.checked_add(n) == Some(most))
    };
    if refuses!(
        site::REGION_PARTIAL_BLOCKS,
        !edge(origin.x, size.width, w, bw) || !edge(origin.y, size.height, h, bh)
    ) {
        return Err("a region of partial blocks");
    }
    if refuses!(
        site::REGION_EMPTY,
        size.width == 0 || size.height == 0 || size.depth == 0
    ) {
        return Err("an empty region");
    }
    Ok(())
}

/// Checks one copy or fill.
fn check_blit(
    command: &BlitCommand<DirectMetal>,
    limits: DeviceLimits,
) -> Result<(), EncodeBlitCommandsError> {
    let out = EncodeBlitCommandsError::OutOfRange;
    let granular = |numbers: &[u64]| {
        numbers
            .iter()
            .all(|n| n.is_multiple_of(BUFFER_COPY_GRANULE))
    };
    match command {
        BlitCommand::CopyFromBufferToBuffer {
            source_buffer,
            source_offset,
            destination_buffer,
            destination_offset,
            size,
        } => {
            if refuses!(
                site::COPY_BUFFERS_NO_BYTES_OR_OFF_WHOLE_WORDS,
                *size == 0 || !granular(&[*source_offset, *destination_offset, *size])
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a copy of no bytes, or at offsets or of a size not in whole words",
                ));
            }
            if refuses!(
                site::COPY_BUFFERS_OUTSIDE_BUFFER,
                !source_buffer.holds(*source_offset, *size)
                    || !destination_buffer.holds(*destination_offset, *size)
            ) {
                return Err(EncodeBlitCommandsError::OutOfRange(
                    "a copy outside its buffer",
                ));
            }
            // Both ends are inside their buffers, so neither sum overflows; one that did would count as overlapping.
            let before = |a: u64, b: u64| b.checked_add(*size).is_none_or(|end| a < end);
            if refuses!(
                site::COPY_BUFFERS_OVERLAPPING_RANGES,
                Rc::ptr_eq(&source_buffer.0, &destination_buffer.0)
                    && before(*source_offset, *destination_offset)
                    && before(*destination_offset, *source_offset)
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a copy between overlapping ranges",
                ));
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
            if refuses!(
                site::COPY_BUFFER_AND_TEXTURE_MULTISAMPLED_OR_DRAWABLE_TEXTURE,
                texture.0.sample_count != 1 || texture.0.framebuffer_only
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a copy between a buffer and a multisampled or drawable texture",
                ));
            }
            let Some((bytes, block)) = found!(
                site::COPY_BUFFER_AND_TEXTURE_ASPECT_FORMAT_LACKS,
                moved(texture.0.pixel_format, *options)
            ) else {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a copy of an aspect the format does not have, or of both at once",
                ));
            };
            check_region(texture, *slice, *level, *origin, *source_size, block).map_err(out)?;
            // Every product and sum below is checked: one that overflows refuses the copy, since its bytes could not
            // lie inside any buffer. A row is at most 2^36 bytes and, past the pitch check, an image at most 2^51, so
            // only the sum of the images can overflow: the rest are checked with the pitches they follow from.
            let blocks_wide = u64::from(source_size.width.div_ceil(block.0));
            let rows = u64::from(source_size.height.div_ceil(block.1));
            let row = blocks_wide.checked_mul(bytes);
            // A row is as long as the longest side of the texture's type (MEASURED: Metal's validation stops 24000 bytes
            // a row of a 3D texture of 8-byte texels, past 2048 of them, found by the fuzzer).
            let longest_side = if texture.0.texture_type == TextureType::Type3D {
                MAX_3D_SIDE
            } else {
                limits.max_side
            };
            let longest_row = longest_side
                .checked_div(block.0)
                .and_then(|blocks| u64::from(blocks).checked_mul(bytes));
            let Some((row, longest_row)) = row.zip(longest_row) else {
                return Err(EncodeBlitCommandsError::OutOfRange(
                    "a copy whose length overflows",
                ));
            };
            if refuses!(
                site::COPY_BUFFER_AND_TEXTURE_ROW_PITCH_SHORT_LONG_OR_OFF_BLOCK,
                *source_bytes_per_row < row
                    || !source_bytes_per_row.is_multiple_of(bytes)
                    || *source_bytes_per_row > longest_row
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a row pitch shorter than a row, longer than the longest, or not a multiple of a block",
                ));
            }
            // The bytes of one image: every row but the last at its pitch, and the last row.
            let image = rows
                .checked_sub(1)
                .and_then(|rows| rows.checked_mul(*source_bytes_per_row))
                .and_then(|n| n.checked_add(row));
            let pitched_image = rows.checked_mul(*source_bytes_per_row);
            let depth = u64::from(source_size.depth);
            let flat = depth == 1 && *source_bytes_per_image == 0;
            if refuses!(
                site::COPY_BUFFER_AND_TEXTURE_IMAGE_PITCH_SHORTER_THAN_IMAGE,
                !flat && pitched_image.is_none_or(|pitched| *source_bytes_per_image < pitched)
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "an image pitch shorter than an image",
                ));
            }
            if refuses!(
                site::COPY_BUFFER_AND_TEXTURE_OFFSET_OFF_BLOCK,
                !source_offset.is_multiple_of(bytes)
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a buffer offset not a multiple of a block",
                ));
            }
            // The bytes of the copy: every image but the last at its pitch, and the last image.
            let Some(length) = found!(
                site::COPY_BUFFER_AND_TEXTURE_LENGTH_THAT_OVERFLOWS,
                image.and_then(|image| {
                    depth
                        .checked_sub(1)
                        .and_then(|images| images.checked_mul(*source_bytes_per_image))
                        .and_then(|n| n.checked_add(image))
                })
            ) else {
                return Err(EncodeBlitCommandsError::OutOfRange(
                    "a copy whose length overflows",
                ));
            };
            if refuses!(
                site::COPY_BUFFER_AND_TEXTURE_OUTSIDE_BUFFER,
                !source_buffer.holds(*source_offset, length)
            ) {
                return Err(EncodeBlitCommandsError::OutOfRange(
                    "a copy outside its buffer",
                ));
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
            if refuses!(
                site::COPY_TEXTURES_FORMATS_OR_SAMPLE_COUNTS_THAT_DIFFER,
                s.pixel_format != d.pixel_format || s.sample_count != d.sample_count
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a copy between textures of formats or sample counts that differ",
                ));
            }
            if refuses!(
                site::COPY_TEXTURES_DRAWABLES_TEXTURE,
                s.framebuffer_only || d.framebuffer_only
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a copy from or into a drawable's texture",
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
            // Both regions are inside their levels, so no sum overflows; one that did would count as not apart.
            let apart = |a: u32, b: u32, n: u32| {
                a.checked_add(n).is_some_and(|end| end <= b)
                    || b.checked_add(n).is_some_and(|end| end <= a)
            };
            let same = |a: u32, b: u32, c: u32, e: u32| match (a.checked_add(b), c.checked_add(e)) {
                (Some(x), Some(y)) => x == y,
                _ => true,
            };
            if refuses!(
                site::COPY_TEXTURES_OVERLAPPING_REGIONS,
                s.storage == d.storage
                    && same(
                        s.base_level,
                        *source_level,
                        d.base_level,
                        *destination_level,
                    )
                    && same(
                        s.base_slice,
                        *source_slice,
                        d.base_slice,
                        *destination_slice,
                    )
                    && !(apart(source_origin.x, destination_origin.x, source_size.width)
                        || apart(source_origin.y, destination_origin.y, source_size.height)
                        || apart(source_origin.z, destination_origin.z, source_size.depth))
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a copy between overlapping regions",
                ));
            }
        }
        BlitCommand::FillBuffer {
            buffer,
            offset,
            length,
            ..
        } => {
            if refuses!(
                site::FILL_NO_BYTES_OR_OFF_WHOLE_WORDS,
                *length == 0 || !granular(&[*offset, *length])
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "a fill of no bytes, or at an offset or of a length not in whole words",
                ));
            }
            if refuses!(site::FILL_OUTSIDE_BUFFER, !buffer.holds(*offset, *length)) {
                return Err(EncodeBlitCommandsError::OutOfRange(
                    "a fill outside its buffer",
                ));
            }
        }
        BlitCommand::GenerateMipmaps { texture } => {
            let t = &texture.0;
            let facts = format_facts(t.pixel_format);
            let filters = facts.resolve && (!facts.float32 || limits.float32_resolve);
            if refuses!(site::MIPMAPS_OF_DRAWABLES_TEXTURE, t.framebuffer_only) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "mipmaps of a drawable's texture",
                ));
            }
            if refuses!(site::MIPMAPS_OF_ONE_LEVEL, t.levels < 2) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "mipmaps of a texture of one level, or of a format not drawn into and filtered",
                ));
            }
            if refuses!(
                site::MIPMAPS_FORMAT_NOT_DRAWN_INTO_AND_FILTERED_OR_MULTISAMPLED,
                !facts.color || !filters || t.sample_count != 1
            ) {
                return Err(EncodeBlitCommandsError::Invalid(
                    "mipmaps of a texture of one level, or of a format not drawn into and filtered",
                ));
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
            let (from, to, size) = (
                ns_uinteger(*source_offset),
                ns_uinteger(*destination_offset),
                ns_uinteger(*size),
            );
            #[expect(unsafe_code, reason = "a copy between buffers")]
            // SAFETY: both ranges lie inside their buffers and do not overlap (check_blit).
            unsafe {
                encoder.copyFromBuffer_sourceOffset_toBuffer_destinationOffset_size(
                    &source_buffer.0.raw,
                    from,
                    &destination_buffer.0.raw,
                    to,
                    size,
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
            let (from, row, image) = (
                ns_uinteger(*source_offset),
                ns_uinteger(*source_bytes_per_row),
                ns_uinteger(*source_bytes_per_image),
            );
            let (size, slice, level, origin, option) = (
                mtl_size(*source_size),
                *destination_slice as NSUInteger,
                *destination_level as NSUInteger,
                mtl_origin(*destination_origin),
                blit_option(*options),
            );
            #[expect(unsafe_code, reason = "a copy from a buffer into a texture")]
            // SAFETY: the region lies inside the level in whole blocks, the bytes inside the buffer at pitches of whole
            // blocks, their length computed without overflow, and the option names an aspect the format has
            // (check_blit).
            unsafe {
                encoder.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin_options(
                    &source_buffer.0.raw, from, row, image, size, &destination_texture.0.raw, slice, level, origin,
                    option,
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
            let (slice, level, origin, size) = (
                *source_slice as NSUInteger,
                *source_level as NSUInteger,
                mtl_origin(*source_origin),
                mtl_size(*source_size),
            );
            let (to, row, image, option) = (
                ns_uinteger(*destination_offset),
                ns_uinteger(*destination_bytes_per_row),
                ns_uinteger(*destination_bytes_per_image),
                blit_option(*options),
            );
            #[expect(unsafe_code, reason = "a copy from a texture into a buffer")]
            // SAFETY: as for a copy into a texture (check_blit).
            unsafe {
                encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage_options(
                    &source_texture.0.raw, slice, level, origin, size, &destination_buffer.0.raw, to, row, image,
                    option,
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
            let (from_slice, from_level, from_origin, size) = (
                *source_slice as NSUInteger,
                *source_level as NSUInteger,
                mtl_origin(*source_origin),
                mtl_size(*source_size),
            );
            let (to_slice, to_level, to_origin) = (
                *destination_slice as NSUInteger,
                *destination_level as NSUInteger,
                mtl_origin(*destination_origin),
            );
            #[expect(unsafe_code, reason = "a copy between textures")]
            // SAFETY: both regions lie inside their levels in whole blocks, of formats and sample counts that agree, and
            // do not overlap (check_blit).
            unsafe {
                encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                    &source_texture.0.raw, from_slice, from_level, from_origin, size, &destination_texture.0.raw,
                    to_slice, to_level, to_origin,
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
                NSRange::new(ns_uinteger(*offset), ns_uinteger(*length)),
                *value,
            );
        }
        BlitCommand::GenerateMipmaps { texture } => {
            encoder.generateMipmapsForTexture(&texture.0.raw);
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
        r.setClearStencil(stencil_value(a.clear_value).unwrap_or(u32::MIN));
    }
    if let Some(buffer) = pass.visibility_result_buffer {
        raw.setVisibilityResultBuffer(Some(&buffer.0.raw));
        raw.setVisibilityResultType(MTLVisibilityResultType::Accumulate);
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
    let module = source.module;
    let entry_index = found!(
        site::FUNCTION_NO_SUCH_ENTRY_POINT,
        module
            .entry_points
            .iter()
            .position(|e| e.name == source.entry_point && e.stage == source.stage)
    )
    .ok_or(MakeFunctionError::Unchecked("no such entry point"))?;
    let entry = &module.entry_points[entry_index];
    if refuses!(
        site::FUNCTION_STAGE_OTHER_THAN_VERTEX_FRAGMENT_AND_COMPUTE,
        !matches!(
            source.stage,
            naga::ShaderStage::Vertex | naga::ShaderStage::Fragment | naga::ShaderStage::Compute
        )
    ) {
        return Err(MakeFunctionError::Unchecked(
            "a stage other than vertex, fragment and compute",
        ));
    }
    let info = source.info.get_entry_point(entry_index);
    let reserved = [source.immediates_buffer_index, source.sizes_buffer_index];
    if refuses!(
        site::FUNCTION_IMMEDIATES_AND_LENGTHS_NOT_AT_TWO_SLOTS_OF_THEIR_OWN,
        reserved.iter().any(|&slot| slot >= MAX_BUFFER_SLOTS) || reserved[0] == reserved[1]
    ) {
        return Err(MakeFunctionError::Unchecked(
            "the immediates and the lengths need two buffer slots of their own",
        ));
    }
    let mut resources = naga::back::msl::BindingMap::default();
    let mut reads = Reads {
        sizes_index: source.sizes_buffer_index,
        workgroup_size: {
            let [width, height, depth] = entry.workgroup_size;
            Size {
                width,
                height,
                depth,
            }
        },
        ..Reads::default()
    };
    let mut buffer_slots = Vec::new();
    let mut layouter = naga::proc::Layouter::default();
    layouter
        .update(module.to_ctx())
        .map_err(|error| MakeFunctionError::Translation(error.to_string()))?;
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
                    layouter[var.ty].alignment,
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
            _ => {}
        }
        if refuses!(
            site::FUNCTION_GLOBAL_IN_ADDRESS_SPACE_NOT_CHECKED,
            !matches!(
                var.space,
                naga::AddressSpace::Uniform
                    | naga::AddressSpace::Storage { .. }
                    | naga::AddressSpace::Handle
            )
        ) {
            return Err(MakeFunctionError::Unchecked(
                "a global in an address space this crate does not check",
            ));
        }
        let Some(binding) = found!(site::FUNCTION_RESOURCE_WITH_NO_BINDING, var.binding) else {
            return Err(MakeFunctionError::Unchecked("a resource with no binding"));
        };
        let Some(slot) = found!(
            site::FUNCTION_RESOURCE_WITH_NO_SLOT,
            source
                .resources
                .iter()
                .find(|r| r.group == binding.group && r.binding == binding.binding)
        ) else {
            return Err(MakeFunctionError::Unchecked("a resource with no slot"));
        };
        let index = slot.index;
        if refuses!(
            site::FUNCTION_RESOURCE_OF_TYPE_NOT_CHECKED,
            !matches!(
                (var.space, ty),
                (
                    naga::AddressSpace::Uniform | naga::AddressSpace::Storage { .. },
                    _
                ) | (
                    _,
                    naga::TypeInner::Image { .. } | naga::TypeInner::Sampler { .. }
                )
            )
        ) {
            return Err(MakeFunctionError::Unchecked(
                "a resource of a type this crate does not check",
            ));
        }
        let mut target = naga::back::msl::BindTarget::default();
        match (var.space, ty) {
            (naga::AddressSpace::Uniform | naga::AddressSpace::Storage { .. }, _) => {
                if refuses!(
                    site::FUNCTION_BUFFER_SLOT_PAST_LAST_OR_TAKEN,
                    index >= MAX_BUFFER_SLOTS
                        || reserved.contains(&index)
                        || source
                            .vertex_buffers
                            .iter()
                            .any(|v| v.buffer_index == index)
                ) {
                    return Err(MakeFunctionError::Unchecked(
                        "a buffer slot past the last or taken",
                    ));
                }
                // A runtime-sized array must hold one element: naga's bound subtracts one element's bytes from the
                // length in 32 bits and wraps on a shorter one (third_party/naga/src/back/msl/writer.rs:1759-1799).
                if refuses!(
                    site::FUNCTION_ARRAY_OF_BINDINGS,
                    matches!(*ty, naga::TypeInner::BindingArray { .. })
                ) {
                    return Err(MakeFunctionError::Unchecked("an array of bindings"));
                }
                let least = match *ty {
                    naga::TypeInner::Struct { ref members, .. }
                        if ty.needs_host_buffer_byte_size(&module.types) =>
                    {
                        members.last().map_or(u64::MIN, |m| {
                            u64::from(m.offset).saturating_add(one_element(module, m.ty))
                        })
                    }
                    naga::TypeInner::Array {
                        size: naga::ArraySize::Dynamic,
                        ..
                    } => one_element(module, var.ty),
                    _ => u64::from(ty.size(module.to_ctx())),
                };
                if let naga::AddressSpace::Storage { access } = var.space {
                    target.mutable = access.contains(naga::StorageAccess::STORE);
                    if target.mutable {
                        reads.written.push(index);
                    }
                }
                target.buffer = Some(slot_number(index));
                buffer_slots.push(index);
                reads
                    .buffers
                    .push((index, least, layouter[var.ty].alignment));
            }
            (
                _,
                naga::TypeInner::Image {
                    dim,
                    arrayed,
                    class,
                },
            ) => {
                if refuses!(
                    site::FUNCTION_TEXTURE_SLOT_PAST_LAST,
                    index >= MAX_TEXTURE_SLOTS
                ) {
                    return Err(MakeFunctionError::Unchecked("a texture slot past the last"));
                }
                if refuses!(
                    site::FUNCTION_TEXTURE_FUNCTION_WRITES,
                    matches!(*class, naga::ImageClass::Storage { access, .. }
                        if access.contains(naga::StorageAccess::STORE)
                            || access.contains(naga::StorageAccess::ATOMIC))
                ) {
                    return Err(MakeFunctionError::Unchecked(
                        "a texture the function writes, whose writes naga does not bound",
                    ));
                }
                let read = match *class {
                    naga::ImageClass::Sampled { multi, kind } => {
                        Some((multi, false, texel_kind(kind)?))
                    }
                    naga::ImageClass::Depth { multi } => Some((multi, true, TexelKind::Float)),
                    naga::ImageClass::Storage { format, .. } => {
                        Some((false, false, storage_texel_kind(format)))
                    }
                    _ => None,
                };
                let Some((multisampled, depth, texel)) =
                    found!(site::FUNCTION_EXTERNAL_TEXTURE, read)
                else {
                    return Err(MakeFunctionError::Unchecked("an external texture"));
                };
                let texture_type = match (dim, arrayed, multisampled) {
                    (naga::ImageDimension::D2, false, false) => Some(TextureType::Type2D),
                    (naga::ImageDimension::D2, false, true) => Some(TextureType::Type2DMultisample),
                    (naga::ImageDimension::D2, true, false) => Some(TextureType::Type2DArray),
                    (naga::ImageDimension::Cube, false, false) => Some(TextureType::TypeCube),
                    (naga::ImageDimension::Cube, true, false) => Some(TextureType::TypeCubeArray),
                    (naga::ImageDimension::D3, false, false) => Some(TextureType::Type3D),
                    _ => None,
                };
                let Some(texture_type) = found!(site::FUNCTION_TEXTURE_TYPE_NOT_MADE, texture_type)
                else {
                    return Err(MakeFunctionError::Unchecked(
                        "a texture type this crate does not make",
                    ));
                };
                target.texture = Some(slot_number(index));
                reads.textures.push((
                    index,
                    TextureRead {
                        texture_type,
                        depth,
                        texel,
                    },
                ));
            }
            (_, naga::TypeInner::Sampler { .. }) => {
                if refuses!(
                    site::FUNCTION_SAMPLER_SLOT_PAST_LAST,
                    index >= MAX_SAMPLER_SLOTS
                ) {
                    return Err(MakeFunctionError::Unchecked("a sampler slot past the last"));
                }
                target.sampler = Some(naga::back::msl::BindSamplerTarget::Resource(slot_number(
                    index,
                )));
                reads.samplers.push(index);
            }
            _ => unreachable!("refused above"),
        }
        resources.insert(binding, target);
    }
    let mut mappings = Vec::new();
    if source.stage == naga::ShaderStage::Vertex {
        for layout in source.vertex_buffers {
            if refuses!(
                site::FUNCTION_VERTEX_BUFFER_SLOT_PAST_LAST_OR_TAKEN,
                layout.buffer_index >= MAX_BUFFER_SLOTS
                    || reserved.contains(&layout.buffer_index)
                    || buffer_slots.contains(&layout.buffer_index)
            ) {
                return Err(MakeFunctionError::Unchecked(
                    "a vertex buffer slot past the last or taken",
                ));
            }
            if refuses!(
                site::FUNCTION_VERTEX_STRIDE_OF_ZERO_OR_PAST_MOST,
                layout.stride == 0 || layout.stride > MAX_VERTEX_STRIDE
            ) {
                return Err(MakeFunctionError::Unchecked(
                    "a vertex buffer of stride zero or past MAX_VERTEX_STRIDE",
                ));
            }
            let mut attributes = Vec::new();
            for attribute in &layout.attributes {
                let format = vertex_format(attribute.format);
                let bytes = u32::try_from(format.size()).unwrap_or(u32::MAX);
                if refuses!(
                    site::FUNCTION_ATTRIBUTE_PAST_ITS_STRIDE,
                    attribute
                        .offset
                        .checked_add(bytes)
                        .is_none_or(|end| end > layout.stride)
                ) {
                    return Err(MakeFunctionError::Unchecked(
                        "an attribute past its vertex buffer's stride",
                    ));
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
                    VertexStepFunction::PerInstance if layout.step_rate == 1 => {
                        naga::back::msl::VertexBufferStepMode::ByInstance
                    }
                    VertexStepFunction::PerInstance => {
                        naga::back::msl::VertexBufferStepMode::ByInstanceRate(layout.step_rate)
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

/// Returns the bytes of one element of an array type, none for another type.
fn one_element(module: &naga::Module, array: naga::Handle<naga::Type>) -> u64 {
    match module.types[array].inner {
        naga::TypeInner::Array { base, .. } => {
            u64::from(module.types[base].inner.size(module.to_ctx()))
        }
        _ => u64::MIN,
    }
}

/// Returns the number type a function reads a sampled texture's texels as.
fn texel_kind(kind: naga::ScalarKind) -> Result<TexelKind, MakeFunctionError> {
    let texel = match kind {
        naga::ScalarKind::Float => Some(TexelKind::Float),
        naga::ScalarKind::Sint => Some(TexelKind::Sint),
        naga::ScalarKind::Uint => Some(TexelKind::Uint),
        _ => None,
    };
    found!(site::FUNCTION_TEXELS_NEITHER_FLOAT_NOR_INTEGER, texel).ok_or(
        MakeFunctionError::Unchecked("a texture of texels neither float nor integer"),
    )
}

/// Returns the number type a function reads a storage texture's texels as.
fn storage_texel_kind(format: naga::StorageFormat) -> TexelKind {
    use naga::StorageFormat as S;
    match format {
        S::R8Uint
        | S::R16Uint
        | S::Rg8Uint
        | S::R32Uint
        | S::Rg16Uint
        | S::Rgba8Uint
        | S::Rgb10a2Uint
        | S::R64Uint
        | S::Rg32Uint
        | S::Rgba16Uint
        | S::Rgba32Uint => TexelKind::Uint,
        S::R8Sint
        | S::R16Sint
        | S::Rg8Sint
        | S::R32Sint
        | S::Rg16Sint
        | S::Rgba8Sint
        | S::Rg32Sint
        | S::Rgba16Sint
        | S::Rgba32Sint => TexelKind::Sint,
        _ => TexelKind::Float,
    }
}

/// Returns a slot as naga's slot type; every slot is below 128, checked before.
fn slot_number(index: u32) -> naga::back::msl::Slot {
    naga::back::msl::Slot::try_from(index).unwrap_or(naga::back::msl::Slot::MAX)
}

/// Returns naga's vertex format of a Metal vertex format.
fn vertex_format(format: VertexFormat) -> naga_types::VertexFormat {
    use naga_types::VertexFormat as N;
    match format {
        VertexFormat::UChar2 => N::Uint8x2,
        VertexFormat::UChar4 => N::Uint8x4,
        VertexFormat::Char2 => N::Sint8x2,
        VertexFormat::Char4 => N::Sint8x4,
        VertexFormat::UChar2Normalized => N::Unorm8x2,
        VertexFormat::UChar4Normalized => N::Unorm8x4,
        VertexFormat::Char2Normalized => N::Snorm8x2,
        VertexFormat::Char4Normalized => N::Snorm8x4,
        VertexFormat::UShort2 => N::Uint16x2,
        VertexFormat::UShort4 => N::Uint16x4,
        VertexFormat::Short2 => N::Sint16x2,
        VertexFormat::Short4 => N::Sint16x4,
        VertexFormat::UShort2Normalized => N::Unorm16x2,
        VertexFormat::UShort4Normalized => N::Unorm16x4,
        VertexFormat::Short2Normalized => N::Snorm16x2,
        VertexFormat::Short4Normalized => N::Snorm16x4,
        VertexFormat::Half2 => N::Float16x2,
        VertexFormat::Half4 => N::Float16x4,
        VertexFormat::Float => N::Float32,
        VertexFormat::Float2 => N::Float32x2,
        VertexFormat::Float3 => N::Float32x3,
        VertexFormat::Float4 => N::Float32x4,
        VertexFormat::UInt => N::Uint32,
        VertexFormat::UInt2 => N::Uint32x2,
        VertexFormat::UInt3 => N::Uint32x3,
        VertexFormat::UInt4 => N::Uint32x4,
        VertexFormat::Int => N::Sint32,
        VertexFormat::Int2 => N::Sint32x2,
        VertexFormat::Int3 => N::Sint32x3,
        VertexFormat::Int4 => N::Sint32x4,
        VertexFormat::UChar => N::Uint8,
        VertexFormat::Char => N::Sint8,
        VertexFormat::UCharNormalized => N::Unorm8,
        VertexFormat::CharNormalized => N::Snorm8,
        VertexFormat::UShort => N::Uint16,
        VertexFormat::Short => N::Sint16,
        VertexFormat::UShortNormalized => N::Unorm16,
        VertexFormat::ShortNormalized => N::Snorm16,
        VertexFormat::Half => N::Float16,
        VertexFormat::UInt1010102Normalized => N::Unorm10_10_10_2,
        VertexFormat::UChar4NormalizedBgra => N::Unorm8x4Bgra,
    }
}

/// What one pixel format is, for the checks; what it takes is the Apple7 column of Apple's feature set tables.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
#[derive(Clone, Copy)]
struct FormatFacts {
    /// The size of one block in bytes, a texel for formats that are not compressed.
    bytes: u32,
    block: (u32, u32),
    depth: bool,
    stencil: bool,
    depth_or_stencil: bool,
    /// The same bits seen as sRGB or as linear values, for formats that have such a twin.
    srgb_twin: Option<PixelFormat>,
    /// A color attachment takes it.
    color: bool,
    /// A color attachment of it blends.
    blend: bool,
    /// A multisampled attachment of it resolves, from the Apple9 family on for 32-bit floats.
    resolve: bool,
    /// Its channels are 32-bit floats, which resolve only from the Apple9 family on.
    float32: bool,
    /// The number type functions read its texels as.
    texel: TexelKind,
}

/// The block of a format that is not compressed: one texel.
///
/// https://developer.apple.com/documentation/metal/mtlpixelformat
const TEXEL_BLOCK: (u32, u32) = (1, 1);

/// The block of the compressed formats this crate makes: four texels by four.
///
/// https://developer.apple.com/documentation/metal/mtlpixelformat/bc1_rgba
const BLOCK_4X4: (u32, u32) = (4, 4);

/// Returns the bytes of a type, the size of a texel or block that holds one.
fn bytes_of<T>() -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(u32::MAX)
}

impl FormatFacts {
    /// Returns whether the format is compressed in blocks of more than one texel.
    fn compressed(&self) -> bool {
        self.block != TEXEL_BLOCK
    }
}

/// Returns what a pixel format is.
fn format_facts(format: PixelFormat) -> FormatFacts {
    use PixelFormat as P;
    let plain = |bytes| FormatFacts {
        bytes,
        block: TEXEL_BLOCK,
        depth: false,
        stencil: false,
        depth_or_stencil: false,
        srgb_twin: None,
        color: true,
        blend: true,
        resolve: true,
        float32: false,
        texel: TexelKind::Float,
    };
    let integer = |bytes, texel| FormatFacts {
        blend: false,
        resolve: false,
        texel,
        ..plain(bytes)
    };
    let (sint, uint) = (TexelKind::Sint, TexelKind::Uint);
    let float32 = |bytes| FormatFacts {
        float32: true,
        ..plain(bytes)
    };
    let twin = |bytes, other| FormatFacts {
        srgb_twin: Some(other),
        ..plain(bytes)
    };
    let blocks = |bytes, (w, h), other: Option<PixelFormat>| FormatFacts {
        block: (w, h),
        srgb_twin: other,
        color: false,
        blend: false,
        resolve: false,
        ..plain(bytes)
    };
    let aspects = |bytes, depth, stencil| FormatFacts {
        depth,
        stencil,
        depth_or_stencil: true,
        color: false,
        blend: false,
        resolve: false,
        ..plain(bytes)
    };
    match format {
        P::A8Unorm | P::R8Snorm => plain(bytes_of::<u8>()),
        P::R8Uint => integer(bytes_of::<u8>(), uint),
        P::R8Sint => integer(bytes_of::<u8>(), sint),
        P::R8Unorm => twin(bytes_of::<u8>(), P::R8UnormSrgb),
        P::R8UnormSrgb => twin(bytes_of::<u8>(), P::R8Unorm),
        P::R16Unorm | P::R16Snorm | P::R16Float => plain(bytes_of::<u16>()),
        P::R16Uint => integer(bytes_of::<u16>(), uint),
        P::R16Sint => integer(bytes_of::<u16>(), sint),
        P::Rg8Unorm | P::Rg8Snorm => plain(bytes_of::<u16>()),
        P::Rg8Uint => integer(bytes_of::<u16>(), uint),
        P::Rg8Sint => integer(bytes_of::<u16>(), sint),
        P::B5g6r5Unorm | P::A1bgr5Unorm | P::Abgr4Unorm | P::Bgr5a1Unorm => {
            plain(bytes_of::<u16>())
        }
        P::R32Uint => integer(bytes_of::<u32>(), uint),
        P::R32Sint => integer(bytes_of::<u32>(), sint),
        P::R32Float => float32(bytes_of::<u32>()),
        P::Rg16Unorm | P::Rg16Snorm | P::Rg16Float => plain(bytes_of::<u32>()),
        P::Rg16Uint => integer(bytes_of::<u32>(), uint),
        P::Rg16Sint => integer(bytes_of::<u32>(), sint),
        P::Rgba8Unorm => twin(bytes_of::<u32>(), P::Rgba8UnormSrgb),
        P::Rgba8UnormSrgb => twin(bytes_of::<u32>(), P::Rgba8Unorm),
        P::Bgra8Unorm => twin(bytes_of::<u32>(), P::Bgra8UnormSrgb),
        P::Bgra8UnormSrgb => twin(bytes_of::<u32>(), P::Bgra8Unorm),
        P::Rgba8Snorm => plain(bytes_of::<u32>()),
        P::Rgba8Uint => integer(bytes_of::<u32>(), uint),
        P::Rgba8Sint => integer(bytes_of::<u32>(), sint),
        P::Rgb10a2Unorm | P::Rg11b10Float | P::Rgb9e5Float => plain(bytes_of::<u32>()),
        P::Rgb10a2Uint => integer(bytes_of::<u32>(), uint),
        P::Rg32Uint => integer(bytes_of::<u64>(), uint),
        P::Rg32Sint => integer(bytes_of::<u64>(), sint),
        P::Rg32Float => float32(bytes_of::<u64>()),
        P::Rgba16Unorm | P::Rgba16Snorm | P::Rgba16Float => plain(bytes_of::<u64>()),
        P::Rgba16Uint => integer(bytes_of::<u64>(), uint),
        P::Rgba16Sint => integer(bytes_of::<u64>(), sint),
        P::Rgba32Uint => integer(bytes_of::<u128>(), uint),
        P::Rgba32Sint => integer(bytes_of::<u128>(), sint),
        P::Rgba32Float => float32(bytes_of::<u128>()),
        P::Bc1Rgba => blocks(bytes_of::<u64>(), BLOCK_4X4, Some(P::Bc1RgbaSrgb)),
        P::Bc1RgbaSrgb => blocks(bytes_of::<u64>(), BLOCK_4X4, Some(P::Bc1Rgba)),
        P::Bc2Rgba => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::Bc2RgbaSrgb)),
        P::Bc2RgbaSrgb => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::Bc2Rgba)),
        P::Bc3Rgba => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::Bc3RgbaSrgb)),
        P::Bc3RgbaSrgb => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::Bc3Rgba)),
        P::Bc4RUnorm | P::Bc4RSnorm => blocks(bytes_of::<u64>(), BLOCK_4X4, None),
        P::Bc5RgUnorm | P::Bc5RgSnorm | P::Bc6hRgbFloat | P::Bc6hRgbUfloat => {
            blocks(bytes_of::<u128>(), BLOCK_4X4, None)
        }
        P::Bc7RgbaUnorm => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::Bc7RgbaUnormSrgb)),
        P::Bc7RgbaUnormSrgb => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::Bc7RgbaUnorm)),
        P::Etc2Rgb8 => blocks(bytes_of::<u64>(), BLOCK_4X4, Some(P::Etc2Rgb8Srgb)),
        P::Etc2Rgb8Srgb => blocks(bytes_of::<u64>(), BLOCK_4X4, Some(P::Etc2Rgb8)),
        P::EacRgba8 => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::EacRgba8Srgb)),
        P::EacRgba8Srgb => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::EacRgba8)),
        P::Astc4x4Ldr => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::Astc4x4Srgb)),
        P::Astc4x4Srgb => blocks(bytes_of::<u128>(), BLOCK_4X4, Some(P::Astc4x4Ldr)),
        P::Depth16Unorm => aspects(bytes_of::<u16>(), true, false),
        P::Depth32Float => aspects(bytes_of::<u32>(), true, false),
        // Functions read stencil as unsigned integers, and depth, of a combined format too, as floats.
        P::Stencil8 => FormatFacts {
            texel: uint,
            ..aspects(bytes_of::<u8>(), false, true)
        },
        P::Depth32FloatStencil8 => aspects(bytes_of::<u64>(), true, true),
        P::X32Stencil8 => FormatFacts {
            texel: uint,
            ..aspects(bytes_of::<u64>(), false, true)
        },
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

/// Returns Metal's swizzle of a view's channels.
fn swizzle_channels(swizzle: TextureSwizzleChannels) -> MTLTextureSwizzleChannels {
    let one = |channel| match channel {
        TextureSwizzle::Zero => MTLTextureSwizzle::Zero,
        TextureSwizzle::One => MTLTextureSwizzle::One,
        TextureSwizzle::Red => MTLTextureSwizzle::Red,
        TextureSwizzle::Green => MTLTextureSwizzle::Green,
        TextureSwizzle::Blue => MTLTextureSwizzle::Blue,
        TextureSwizzle::Alpha => MTLTextureSwizzle::Alpha,
    };
    MTLTextureSwizzleChannels {
        red: one(swizzle.red),
        green: one(swizzle.green),
        blue: one(swizzle.blue),
        alpha: one(swizzle.alpha),
    }
}

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

/// Returns whether a blend factor reads the fragment function's second color output.
fn reads_second_source(factor: BlendFactor) -> bool {
    matches!(
        factor,
        BlendFactor::Source1Color
            | BlendFactor::OneMinusSource1Color
            | BlendFactor::Source1Alpha
            | BlendFactor::OneMinusSource1Alpha
    )
}

/// Returns Metal's blend factor of a blend factor.
fn map_blend_factor(factor: BlendFactor) -> MTLBlendFactor {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    /// Fails when a check's site is not listed, a listed site is not checked once, or two sites share a name or label.
    ///
    /// A constant of `site` declared outside `sites!` is either checked nowhere, which the dead code lint refuses, or
    /// checked, which the assertion in `refuses!` and `found!` refuses as the crate is built, and which this test finds
    /// too. The source is read without its spaces and line breaks, so that no formatting changes what it finds.
    #[test]
    fn every_site_checked_is_listed_and_every_listed_site_is_checked_once() {
        let source: String = include_str!("implementation.rs")
            .split_whitespace()
            .collect();
        let mut checked: BTreeMap<&str, usize> = BTreeMap::new();
        // Spelled in parts, so that this test is not a check.
        for needle in [concat!("refuses", "!(site::"), concat!("found", "!(site::")] {
            for (at, _) in source.match_indices(needle) {
                let rest = source
                    .get(at.saturating_add(needle.len())..)
                    .unwrap_or_default();
                let name = rest.split(',').next().unwrap_or_default();
                let count = checked.entry(name).or_default();
                *count = count.saturating_add(1);
            }
        }
        let listed: BTreeMap<&str, &str> = crate::site::ALL.iter().copied().collect();
        let labels: std::collections::BTreeSet<&str> =
            crate::site::ALL.iter().map(|(_, label)| *label).collect();
        assert_eq!(
            listed.len(),
            crate::site::ALL.len(),
            "a site's name listed twice"
        );
        assert_eq!(
            labels.len(),
            crate::site::ALL.len(),
            "a label given to two sites"
        );
        let unlisted: Vec<&&str> = checked
            .keys()
            .filter(|name| !listed.contains_key(*name))
            .collect();
        assert!(unlisted.is_empty(), "checked but not listed: {unlisted:?}");
        let unchecked: Vec<&&str> = listed
            .keys()
            .filter(|name| !checked.contains_key(*name))
            .collect();
        assert!(
            unchecked.is_empty(),
            "listed but checked nowhere: {unchecked:?}"
        );
        let twice: Vec<(&&str, &usize)> = checked.iter().filter(|(_, count)| **count > 1).collect();
        assert!(
            twice.is_empty(),
            "checked at more than one place: {twice:?}"
        );
    }

    /// Fails when the constant check that every site is listed would take a label that is not.
    #[test]
    fn a_label_not_in_the_list_is_not_listed() {
        assert!(
            crate::site::ALL
                .iter()
                .all(|(_, label)| crate::site::listed(label))
        );
        assert!(!crate::site::listed(
            "pipeline: a site declared outside the list"
        ));
        assert!(!crate::site::listed(""));
    }
}
