//! The tests every implementation of wcontainers-metal runs, written against its traits alone.

use wcontainers_metal::{
    BlendFactor, BlendOperation, Blending, BlitCommand, BlitOption, ColorWriteMask,
    CommandBufferStatus, CommitError, CompareFunction, ComputeCommand, DISPATCH_ARGUMENTS_BYTES,
    DRAW_ARGUMENTS_BYTES, DRAW_INDEXED_ARGUMENTS_BYTES, DepthStencilDescriptor,
    DispatchThreadgroupsIndirectArguments, DrawIndexedPrimitivesIndirectArguments,
    DrawPrimitivesIndirectArguments, EncodeBlitCommandsError, EncodeComputeCommandsError,
    EncodeRenderCommandsError, FunctionSource, INDIRECT_ARGUMENTS_ALIGNMENT, IndexType, LoadAction,
    MAX_COLOR_ATTACHMENTS, MAX_TEXTURE_SIDE, MAX_VERTEX_STRIDE, MakeBufferError,
    MakeRenderPipelineStateError, MakeSamplerStateError, MakeTextureError, Metal, Origin,
    PixelFormat, PrimitiveTopologyClass, PrimitiveType, ReadContentsError, RenderCommand,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipelineColorAttachmentDescriptor, RenderPipelineDescriptor, ResolveTarget, ResourceSlot,
    SamplerAddressMode, SamplerBorderColor, SamplerDescriptor, SamplerMinMagFilter,
    SamplerMipFilter, ScissorRect, Size, StencilDescriptor, StencilOperation, StoreAction,
    TextureDescriptor, TextureSwizzle, TextureSwizzleChannels, TextureType, TextureUsage,
    TextureViewDescriptor, VISIBILITY_RESULT_BYTES, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexStepFunction, Viewport, VisibilityResultMode, WaitUntilCompletedError,
    WriteContentsError,
};

/// The side of the square textures the tests draw into and copy, in texels: one block of a compressed format.
///
/// https://developer.apple.com/documentation/metal/mtlpixelformat/bc1_rgba
const TARGET_SIDE: u32 = 4;

/// The bytes of one `rgba8Unorm` texel.
///
/// https://developer.apple.com/documentation/metal/mtlpixelformat/rgba8unorm
const RGBA8_BYTES: u64 = 4;

/// The buffer slot of the tests' immediates, which no test reads: the last but one of a stage's 31.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const IMMEDIATES: u32 = 29;

/// The buffer slot of the lengths the bounds checks read: the last of a stage's 31.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const SIZES: u32 = 30;

/// The buffer slot of the tests' one vertex buffer, among a stage's 31.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const VERTICES: u32 = 20;

/// The tests' functions: a triangle in a uniform's color or in whole numbers, one sampling a texture, and two computes.
const PROGRAMS: &str = "
struct Color { value: vec4<f32> }
@group(0) @binding(0) var<uniform> color: Color;
@group(0) @binding(16) var samp: sampler;
@group(0) @binding(32) var tex: texture_2d<f32>;
@group(0) @binding(48) var<storage, read_write> words: array<u32>;
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn flat() -> @location(0) vec4<f32> {
    return color.value;
}
@fragment fn whole() -> @location(0) vec4<u32> {
    return vec4<u32>(1u);
}
@fragment fn sampled(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, at.xy / 4.0);
}
@compute @workgroup_size(4) fn double(@builtin(global_invocation_id) id: vec3<u32>) {
    words[id.x] = words[id.x] * 2u;
}
@compute @workgroup_size(1) fn spin() {
    var x = words[0];
    for (var i = 0u; i < words[1]; i++) {
        x = x * 1664525u + 1013904223u;
    }
    words[2] = x;
}
var<workgroup> total: atomic<u32>;
@compute @workgroup_size(4) fn sum(@builtin(local_invocation_index) i: u32) {
    atomicAdd(&total, words[i]);
    workgroupBarrier();
    if i == 0u {
        words[4] = atomicLoad(&total);
    }
}
";

/// The count Metal gives a texture's depth, levels, samples and array length unless told otherwise, and so one of each.
///
/// https://developer.apple.com/documentation/metal/mtltexturedescriptor
const SINGLE: u32 = 1;

/// The buffer slot of the words the compute tests read, among a stage's 31.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const WORDS_SLOT: u32 = 1;

/// A byte the tests fill buffers with, which no zeroed buffer holds.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/fill(buffer:range:value:)
const FILL_BYTE: u8 = 1;

/// The tests' vertex function, which places the covering triangle.
const VERTEX: &str = "vs";

/// The fragment function that draws the uniform's color.
const FLAT: &str = "flat";

/// The fragment function that draws in whole numbers.
const WHOLE: &str = "whole";

/// The fragment function that samples the texture.
const SAMPLED: &str = "sampled";

/// The compute function that doubles each word.
const DOUBLE: &str = "double";

/// The compute function that sums words through workgroup memory.
const SUM: &str = "sum";

/// A triangle covering every pixel centre of the target: corners at x and y of -1 and 3 in clip space.
///
/// https://developer.apple.com/documentation/metal/mtlviewport
const COVERING: &[[f32; 2]] = &[[-1.0, -1.0], [3.0, -1.0], [-1.0, 3.0]];

/// The vertices of the covering triangle, which one draw of a triangle takes.
const COVERING_VERTICES: u32 = COVERING.len() as u32;

/// The bytes of the covering triangle's positions.
const COVERING_BYTES: u64 = size_of_val(COVERING) as u64;

/// The bytes of one color of four floats.
const COLOR_BYTES: u64 = size_of::<Color>() as u64;

/// The bytes of the words the compute tests read: eight, the fifth of which `sum` writes.
const WORDS_BYTES: u64 = (WORD_COUNT * size_of::<u32>()) as u64;

/// The words the compute tests read: two threadgroups of four, as their `workgroup_size` says.
///
/// https://www.w3.org/TR/WGSL/#attribute-workgroup_size
const WORD_COUNT: usize = 8;

/// One color as the tests' functions read it, a `vec4<f32>`.
type Color = [f32; COLOR_CHANNELS];

/// The channels of a color: red, green, blue and alpha.
///
/// https://www.w3.org/TR/WGSL/#vector-types
const COLOR_CHANNELS: usize = 4;

/// Opaque green, the color the covering draws write.
///
/// https://www.w3.org/TR/WGSL/#vector-types
const GREEN: Color = [0.0, 1.0, 0.0, 1.0];

/// Returns the bytes of a color as a function reads it from a buffer.
fn color_bytes(color: Color) -> Vec<u8> {
    color
        .iter()
        .flat_map(|channel| channel.to_le_bytes())
        .collect()
}

/// Returns the texel a color is stored as in an `RGBA8Unorm` texture: each channel scaled to a byte and rounded.
///
/// https://developer.apple.com/documentation/metal/mtlpixelformat/rgba8unorm
fn rgba8_unorm(color: Color) -> [u8; COLOR_CHANNELS] {
    color.map(|channel| (channel * f32::from(u8::MAX)).round() as u8)
}

/// The slots of the tests' resources: the uniform, the sampler, the texture and the storage buffer.
///
/// https://www.w3.org/TR/WGSL/#attribute-binding
const SLOTS: &[ResourceSlot] = &[
    ResourceSlot {
        group: u32::MIN,
        binding: u32::MIN,
        index: u32::MIN,
    },
    ResourceSlot {
        group: u32::MIN,
        binding: 16,
        index: u32::MIN,
    },
    ResourceSlot {
        group: u32::MIN,
        binding: 32,
        index: u32::MIN,
    },
    ResourceSlot {
        group: u32::MIN,
        binding: 48,
        index: WORDS_SLOT,
    },
];

/// The viewport of the whole target, over every depth from the nearest to the farthest.
///
/// https://developer.apple.com/documentation/metal/mtlviewport
const TARGET_VIEWPORT: Viewport = Viewport {
    origin_x: 0.0,
    origin_y: 0.0,
    width: TARGET_SIDE as f64,
    height: TARGET_SIDE as f64,
    znear: 0.0,
    zfar: 1.0,
};

/// Bytes no test writes by accident.
fn pattern(length: usize) -> Vec<u8> {
    (0..length)
        .map(|i| (i as u8).wrapping_mul(37).wrapping_add(11))
        .collect()
}

/// Parses and validates the tests' functions.
fn programs() -> (naga::Module, naga::valid::ModuleInfo) {
    let module = naga::front::wgsl::parse_str(PROGRAMS).expect("the test functions parse");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the test functions validate");
    (module, info)
}

/// Compiles one entry point of the tests' functions.
fn function<M: Metal>(subject: &mut M, stage: naga::ShaderStage, entry_point: &str) -> M::Function {
    let (module, info) = programs();
    let vertex_buffers = [VertexBufferLayout {
        buffer_index: VERTICES,
        stride: 8,
        step_function: VertexStepFunction::PerVertex,
        step_rate: 1,
        attributes: vec![VertexAttribute {
            location: u32::MIN,
            offset: u32::MIN,
            format: VertexFormat::Float2,
        }],
    }];
    subject
        .make_function(&FunctionSource {
            module: &module,
            info: &info,
            stage,
            entry_point,
            resources: SLOTS,
            immediates_buffer_index: IMMEDIATES,
            sizes_buffer_index: SIZES,
            vertex_buffers: &vertex_buffers,
            point_size: false,
        })
        .expect("Metal answers make function")
}

/// Makes an `rgba8Unorm` texture of `TARGET_SIDE` by `TARGET_SIDE` the tests draw into, copy and sample.
fn target<M: Metal>(subject: &mut M) -> M::Texture {
    subject
        .make_texture(&TextureDescriptor {
            texture_type: TextureType::Type2D,
            pixel_format: PixelFormat::Rgba8Unorm,
            width: TARGET_SIDE,
            height: TARGET_SIDE,
            depth: SINGLE,
            mipmap_level_count: SINGLE,
            sample_count: SINGLE,
            array_length: SINGLE,
            usage: TextureUsage {
                shader_read: true,
                shader_write: false,
                render_target: true,
                pixel_format_view: false,
            },
        })
        .expect("Metal answers make texture")
}

/// Makes a buffer holding `bytes`.
fn filled<M: Metal>(subject: &mut M, bytes: &[u8]) -> M::Buffer {
    let buffer = subject
        .make_buffer(bytes.len() as u64)
        .expect("Metal answers make buffer");
    subject
        .write_contents(&buffer, u64::MIN, bytes)
        .expect("Metal answers write contents");
    buffer
}

/// Commits a command buffer, waits for it, and checks that it completed.
fn finish<M: Metal>(subject: &mut M, command_buffer: &M::CommandBuffer) {
    subject
        .commit(command_buffer)
        .expect("Metal answers commit");
    subject
        .wait_until_completed(command_buffer)
        .expect("Metal answers wait until completed");
    assert_eq!(
        subject.status(command_buffer),
        CommandBufferStatus::Completed
    );
}

/// Copies a `TARGET_SIDE` by `TARGET_SIDE` `rgba8Unorm` texture into a new buffer and returns its bytes.
fn read_texture<M: Metal>(subject: &mut M, texture: &M::Texture) -> Vec<u8> {
    let row = u64::from(TARGET_SIDE) * RGBA8_BYTES;
    let buffer = subject
        .make_buffer(row * u64::from(TARGET_SIDE))
        .expect("Metal answers make buffer");
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::CopyFromTextureToBuffer {
                source_texture: texture.clone(),
                source_slice: u32::MIN,
                source_level: u32::MIN,
                source_origin: Origin::default(),
                source_size: Size {
                    width: TARGET_SIDE,
                    height: TARGET_SIDE,
                    depth: SINGLE,
                },
                destination_buffer: buffer.clone(),
                destination_offset: u64::MIN,
                destination_bytes_per_row: row,
                destination_bytes_per_image: row * u64::from(TARGET_SIDE),
                options: BlitOption::None,
            }],
        )
        .expect("Metal answers encode blit commands");
    finish(subject, &command_buffer);
    let mut bytes = vec![u8::MIN; (row * u64::from(TARGET_SIDE)) as usize];
    subject
        .read_contents(&buffer, u64::MIN, &mut bytes)
        .expect("Metal answers read contents");
    bytes
}

/// Makes a pipeline drawing `fragment` into one `rgba8Unorm` target.
fn pipeline<M: Metal>(subject: &mut M, fragment: &str) -> M::RenderPipelineState {
    let vertex = function(subject, naga::ShaderStage::Vertex, VERTEX);
    let fragment = function(subject, naga::ShaderStage::Fragment, fragment);
    let mut color_attachments = [None; MAX_COLOR_ATTACHMENTS];
    color_attachments[0] = Some(RenderPipelineColorAttachmentDescriptor {
        pixel_format: PixelFormat::Rgba8Unorm,
        blending: None,
        write_mask: ColorWriteMask {
            red: true,
            green: true,
            blue: true,
            alpha: true,
        },
    });
    subject
        .make_render_pipeline_state(&RenderPipelineDescriptor {
            vertex_function: &vertex,
            fragment_function: Some(&fragment),
            color_attachments,
            depth_attachment_pixel_format: None,
            stencil_attachment_pixel_format: None,
            raster_sample_count: SINGLE,
            alpha_to_coverage_enabled: false,
            rasterization_enabled: true,
            input_primitive_topology: PrimitiveTopologyClass::Triangle,
        })
        .expect("Metal answers make render pipeline state")
}

/// Returns a pass drawing into `texture`, cleared to transparent black.
fn pass<M: Metal>(texture: &M::Texture) -> RenderPassDescriptor<'_, M> {
    let mut color_attachments = [const { None }; MAX_COLOR_ATTACHMENTS];
    color_attachments[0] = Some(RenderPassColorAttachment {
        texture,
        level: u32::MIN,
        slice: u32::MIN,
        depth_plane: u32::MIN,
        load_action: LoadAction::Clear,
        store_action: StoreAction::Store,
        clear_color: [0.0; 4],
        resolve: None,
    });
    RenderPassDescriptor {
        color_attachments,
        depth_attachment: None,
        stencil_attachment: None,
        visibility_result_buffer: None,
        render_target_width: TARGET_SIDE,
        render_target_height: TARGET_SIDE,
        default_raster_sample_count: SINGLE,
    }
}

/// Returns the commands that draw the covering triangle with `pipeline`, after `bindings`.
fn covering_draw<M: Metal>(
    pipeline: &M::RenderPipelineState,
    vertices: &M::Buffer,
    depth_stencil: &M::DepthStencilState,
    bindings: Vec<RenderCommand<M>>,
) -> Vec<RenderCommand<M>> {
    let mut commands = vec![
        RenderCommand::SetRenderPipelineState(pipeline.clone()),
        RenderCommand::SetDepthStencilState(depth_stencil.clone()),
        RenderCommand::SetVertexBuffer {
            buffer: vertices.clone(),
            offset: u64::MIN,
            length: COVERING_BYTES,
            index: VERTICES,
        },
        RenderCommand::SetViewports(vec![TARGET_VIEWPORT]),
        RenderCommand::SetScissorRects(vec![ScissorRect {
            x: u32::MIN,
            y: u32::MIN,
            width: TARGET_SIDE,
            height: TARGET_SIDE,
        }]),
    ];
    commands.extend(bindings);
    commands.push(RenderCommand::DrawPrimitives {
        primitive_type: PrimitiveType::Triangle,
        vertex_start: u32::MIN,
        vertex_count: COVERING_VERTICES,
        instance_count: SINGLE,
        base_instance: u32::MIN,
    });
    commands
}

/// Returns a depth and stencil state that tests nothing.
fn no_depth<M: Metal>(subject: &mut M) -> M::DepthStencilState {
    subject
        .make_depth_stencil_state(&DepthStencilDescriptor {
            depth_compare_function: CompareFunction::Always,
            depth_write_enabled: false,
            front_face_stencil: None,
            back_face_stencil: None,
        })
        .expect("Metal answers make depth stencil state")
}

/// Checks that the device has a name.
pub fn has_a_name<M: Metal>(subject: M) {
    assert!(!subject.name().is_empty());
}

/// The bytes of the buffer the contents tests write and read.
///
/// https://developer.apple.com/documentation/metal/mtlbuffer/contents()
const CONTENTS_BYTES: u64 = 64;

/// Where the contents test writes `CONTENTS_WRITTEN`: off every word, so that no alignment is assumed.
///
/// https://developer.apple.com/documentation/metal/mtlbuffer/contents()
const CONTENTS_WRITE_OFFSET: u64 = 3;

/// The bytes the contents test writes.
///
/// https://developer.apple.com/documentation/metal/mtlbuffer/contents()
const CONTENTS_WRITTEN: [u8; 3] = [1, 2, 3];

/// Checks that bytes written into a buffer read back, and that ranges past its end are refused.
pub fn writes_and_reads_buffer_contents<M: Metal>(mut subject: M) {
    let bytes = pattern(CONTENTS_BYTES as usize);
    let buffer = filled(&mut subject, &bytes);
    subject
        .write_contents(&buffer, CONTENTS_WRITE_OFFSET, &CONTENTS_WRITTEN)
        .expect("Metal answers write contents");
    let mut expected = bytes.clone();
    let start = CONTENTS_WRITE_OFFSET as usize;
    expected[start..start + CONTENTS_WRITTEN.len()].copy_from_slice(&CONTENTS_WRITTEN);
    let mut read = vec![u8::MIN; CONTENTS_BYTES as usize];
    subject
        .read_contents(&buffer, u64::MIN, &mut read)
        .expect("Metal answers read contents");
    assert_eq!(read, expected);
    // The same write ending one byte past the end, and a read starting one byte past it.
    let past = CONTENTS_BYTES - CONTENTS_WRITTEN.len() as u64 + 1;
    assert_eq!(
        subject.write_contents(&buffer, past, &CONTENTS_WRITTEN),
        Err(WriteContentsError::OutOfRange)
    );
    assert_eq!(
        subject.read_contents(&buffer, CONTENTS_BYTES + 1, &mut [u8::MIN; 1]),
        Err(ReadContentsError::OutOfRange)
    );
}

/// Checks that a buffer an uncommitted command buffer uses is refused to the CPU, and a command buffer committed twice.
pub fn refuses_contents_of_uncommitted_work_and_a_second_commit<M: Metal>(mut subject: M) {
    let buffer = filled(&mut subject, &pattern(16));
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    assert_eq!(
        subject.wait_until_completed(&command_buffer),
        Err(WaitUntilCompletedError::NotCommitted)
    );
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::FillBuffer {
                buffer: buffer.clone(),
                offset: u64::MIN,
                length: 4,
                value: 9,
            }],
        )
        .expect("Metal answers encode blit commands");
    assert_eq!(
        subject.write_contents(&buffer, u64::MIN, &[u8::MIN]),
        Err(WriteContentsError::UsedByUncommittedWork)
    );
    finish(&mut subject, &command_buffer);
    assert_eq!(
        subject.commit(&command_buffer),
        Err(CommitError::AlreadyCommitted)
    );
    let mut read = [u8::MIN; 4];
    subject
        .read_contents(&buffer, u64::MIN, &mut read)
        .expect("Metal answers read contents");
    assert_eq!(read, [9; 4]);
}

/// Checks that a buffer whose only user was a command buffer dropped uncommitted is the CPU's again.
pub fn frees_a_buffer_whose_command_buffer_was_dropped<M: Metal>(mut subject: M) {
    let buffer = filled(&mut subject, &pattern(COLOR_BYTES as usize));
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::FillBuffer {
                buffer: buffer.clone(),
                offset: u64::MIN,
                length: COLOR_BYTES,
                value: FILL_BYTE,
            }],
        )
        .expect("Metal answers encode blit commands");
    drop(command_buffer);
    subject
        .write_contents(&buffer, u64::MIN, &[u8::MIN; COLOR_BYTES as usize])
        .expect("Metal answers write contents");
}

/// The bytes of the buffer the copy and fill test writes into.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy(from:sourceoffset:to:destinationoffset:size:)
const COPY_BYTES: usize = 32;

/// Where the copy test reads, writes and how much, each a multiple of four bytes as macOS's copies take them.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy(from:sourceoffset:to:destinationoffset:size:)
const COPY_FROM: usize = 4;

/// Where the copy test writes.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy(from:sourceoffset:to:destinationoffset:size:)
const COPY_TO: usize = 8;

/// How many bytes the copy test copies.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy(from:sourceoffset:to:destinationoffset:size:)
const COPY_LENGTH: usize = 8;

/// Where the fill test fills, past the copied bytes, a multiple of four as macOS's fills take it.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/fill(buffer:range:value:)
const FILL_AT: usize = 20;

/// How many bytes the fill test fills.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/fill(buffer:range:value:)
const FILL_LENGTH: usize = 4;

/// The byte the fill test fills with, unlike any of `pattern`'s neighbours.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/fill(buffer:range:value:)
const FILL_VALUE: u8 = 0xab;

/// Checks that copies between buffers and a texture, and a fill, move exactly the bytes they name.
pub fn copies_and_fills_move_exact_bytes<M: Metal>(mut subject: M) {
    let row = u64::from(TARGET_SIDE) * RGBA8_BYTES;
    let bytes = pattern((row * u64::from(TARGET_SIDE)) as usize);
    let source = filled(&mut subject, &bytes);
    let texture = target(&mut subject);
    let copy = filled(&mut subject, &[u8::MIN; COPY_BYTES]);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[
                BlitCommand::CopyFromBufferToTexture {
                    source_buffer: source.clone(),
                    source_offset: u64::MIN,
                    source_bytes_per_row: row,
                    source_bytes_per_image: row * u64::from(TARGET_SIDE),
                    source_size: Size {
                        width: TARGET_SIDE,
                        height: TARGET_SIDE,
                        depth: SINGLE,
                    },
                    destination_texture: texture.clone(),
                    destination_slice: u32::MIN,
                    destination_level: u32::MIN,
                    destination_origin: Origin::default(),
                    options: BlitOption::None,
                },
                BlitCommand::CopyFromBufferToBuffer {
                    source_buffer: source.clone(),
                    source_offset: COPY_FROM as u64,
                    destination_buffer: copy.clone(),
                    destination_offset: COPY_TO as u64,
                    size: COPY_LENGTH as u64,
                },
                BlitCommand::FillBuffer {
                    buffer: copy.clone(),
                    offset: FILL_AT as u64,
                    length: FILL_LENGTH as u64,
                    value: FILL_VALUE,
                },
            ],
        )
        .expect("Metal answers encode blit commands");
    finish(&mut subject, &command_buffer);
    assert_eq!(read_texture(&mut subject, &texture), bytes);
    let mut expected = vec![u8::MIN; COPY_BYTES];
    expected[COPY_TO..COPY_TO + COPY_LENGTH]
        .copy_from_slice(&bytes[COPY_FROM..COPY_FROM + COPY_LENGTH]);
    expected[FILL_AT..FILL_AT + FILL_LENGTH].fill(FILL_VALUE);
    let mut read = vec![u8::MIN; COPY_BYTES];
    subject
        .read_contents(&copy, u64::MIN, &mut read)
        .expect("Metal answers read contents");
    assert_eq!(read, expected);
}

/// Checks that a triangle drawn in a uniform's color covers the target, and a draw missing its uniform is refused.
pub fn draws_in_a_uniform_color_and_refuses_an_empty_slot<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let depth_stencil = no_depth(&mut subject);
    let vertices = filled(
        &mut subject,
        &COVERING
            .as_flattened()
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let color = filled(
        &mut subject,
        &[0.0f32, 1.0, 0.0, 1.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let texture = target(&mut subject);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    let missing = covering_draw(&state, &vertices, &depth_stencil, Vec::new());
    assert!(matches!(
        subject.encode_render_commands(&command_buffer, &pass(&texture), &missing),
        Err(EncodeRenderCommandsError::Unbound(_))
    ));
    let bound = vec![RenderCommand::SetFragmentBuffer {
        buffer: color,
        offset: u64::MIN,
        length: COLOR_BYTES,
        index: u32::MIN,
    }];
    let commands = covering_draw(&state, &vertices, &depth_stencil, bound);
    subject
        .encode_render_commands(&command_buffer, &pass(&texture), &commands)
        .expect("Metal answers encode render commands");
    finish(&mut subject, &command_buffer);
    assert_eq!(
        read_texture(&mut subject, &texture),
        rgba8_unorm(GREEN).repeat((TARGET_SIDE * TARGET_SIDE) as usize)
    );
}

/// Checks that a draw sampling a view of a texture through a nearest sampler reads each texel.
pub fn samples_a_texture_view<M: Metal>(mut subject: M) {
    let row = u64::from(TARGET_SIDE) * RGBA8_BYTES;
    let bytes = pattern((row * u64::from(TARGET_SIDE)) as usize);
    let source = filled(&mut subject, &bytes);
    let texture = target(&mut subject);
    let view = subject
        .make_texture_view(
            &texture,
            &TextureViewDescriptor {
                pixel_format: PixelFormat::Rgba8Unorm,
                texture_type: TextureType::Type2D,
                first_level: u32::MIN,
                level_count: SINGLE,
                first_slice: u32::MIN,
                slice_count: SINGLE,
                swizzle: TextureSwizzleChannels::IDENTITY,
            },
        )
        .expect("Metal answers make texture view");
    let sampler = subject
        .make_sampler_state(&SamplerDescriptor {
            min_filter: SamplerMinMagFilter::Nearest,
            mag_filter: SamplerMinMagFilter::Nearest,
            mip_filter: SamplerMipFilter::NotMipmapped,
            s_address_mode: SamplerAddressMode::ClampToEdge,
            t_address_mode: SamplerAddressMode::ClampToEdge,
            r_address_mode: SamplerAddressMode::ClampToEdge,
            border_color: SamplerBorderColor::TransparentBlack,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            max_anisotropy: SINGLE,
            compare_function: None,
        })
        .expect("Metal answers make sampler state");
    let state = pipeline(&mut subject, SAMPLED);
    let depth_stencil = no_depth(&mut subject);
    let vertices = filled(
        &mut subject,
        &COVERING
            .as_flattened()
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let drawn = target(&mut subject);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::CopyFromBufferToTexture {
                source_buffer: source,
                source_offset: u64::MIN,
                source_bytes_per_row: row,
                source_bytes_per_image: row * u64::from(TARGET_SIDE),
                source_size: Size {
                    width: TARGET_SIDE,
                    height: TARGET_SIDE,
                    depth: SINGLE,
                },
                destination_texture: texture,
                destination_slice: u32::MIN,
                destination_level: u32::MIN,
                destination_origin: Origin::default(),
                options: BlitOption::None,
            }],
        )
        .expect("Metal answers encode blit commands");
    let bound = vec![
        RenderCommand::SetFragmentTexture {
            texture: view,
            index: u32::MIN,
        },
        RenderCommand::SetFragmentSamplerState {
            sampler,
            index: u32::MIN,
        },
    ];
    let commands = covering_draw(&state, &vertices, &depth_stencil, bound);
    subject
        .encode_render_commands(&command_buffer, &pass(&drawn), &commands)
        .expect("Metal answers encode render commands");
    finish(&mut subject, &command_buffer);
    assert_eq!(read_texture(&mut subject, &drawn), bytes);
}

/// Returns the first texel of a draw that samples `view_of` a texture filled with `texel`.
///
/// The texture is a `TARGET_SIDE` square of `pixel_format`; the covering triangle samples the view into an
/// `rgba8Unorm` target.
fn sampled_texel<M: Metal>(
    subject: &mut M,
    pixel_format: PixelFormat,
    texel: &[u8],
    view_of: impl FnOnce(&mut M, &M::Texture) -> M::Texture,
) -> [u8; 4] {
    let row = u64::from(TARGET_SIDE) * texel.len() as u64;
    let source = filled(subject, &texel.repeat((TARGET_SIDE * TARGET_SIDE) as usize));
    let texture = subject
        .make_texture(&TextureDescriptor {
            usage: TextureUsage {
                shader_read: true,
                shader_write: false,
                render_target: false,
                pixel_format_view: true,
            },
            ..described(TextureType::Type2D, pixel_format)
        })
        .expect("Metal answers make texture");
    let view = view_of(subject, &texture);
    let sampler = subject
        .make_sampler_state(&SamplerDescriptor {
            min_filter: SamplerMinMagFilter::Nearest,
            mag_filter: SamplerMinMagFilter::Nearest,
            mip_filter: SamplerMipFilter::NotMipmapped,
            s_address_mode: SamplerAddressMode::ClampToEdge,
            t_address_mode: SamplerAddressMode::ClampToEdge,
            r_address_mode: SamplerAddressMode::ClampToEdge,
            border_color: SamplerBorderColor::TransparentBlack,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            max_anisotropy: SINGLE,
            compare_function: None,
        })
        .expect("Metal answers make sampler state");
    let state = pipeline(subject, SAMPLED);
    let depth_stencil = no_depth(subject);
    let vertices = covering(subject);
    let drawn = target(subject);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::CopyFromBufferToTexture {
                source_buffer: source,
                source_offset: u64::MIN,
                source_bytes_per_row: row,
                source_bytes_per_image: row * u64::from(TARGET_SIDE),
                source_size: Size {
                    width: TARGET_SIDE,
                    height: TARGET_SIDE,
                    depth: SINGLE,
                },
                destination_texture: texture,
                destination_slice: u32::MIN,
                destination_level: u32::MIN,
                destination_origin: Origin::default(),
                options: BlitOption::None,
            }],
        )
        .expect("Metal answers encode blit commands");
    let bound = vec![
        RenderCommand::SetFragmentTexture {
            texture: view,
            index: u32::MIN,
        },
        RenderCommand::SetFragmentSamplerState {
            sampler,
            index: u32::MIN,
        },
    ];
    let commands = covering_draw(&state, &vertices, &depth_stencil, bound);
    subject
        .encode_render_commands(&command_buffer, &pass(&drawn), &commands)
        .expect("Metal answers encode render commands");
    finish(subject, &command_buffer);
    let texels = read_texture(subject, &drawn);
    [texels[0], texels[1], texels[2], texels[3]]
}

/// A 16-bit texel of four different 4-bit channels: 1 to 4 from its high bits down.
///
/// https://developer.apple.com/documentation/metal/mtlpixelformat/abgr4unorm
const NIBBLES: u16 = 0x1234;

/// Checks where `abgr4Unorm` holds its channels: measured, alpha in the low four bits, then blue, green and red.
pub fn samples_abgr4_with_alpha_in_its_low_bits<M: Metal>(mut subject: M) {
    let seen = sampled_texel(
        &mut subject,
        PixelFormat::Abgr4Unorm,
        &NIBBLES.to_le_bytes(),
        |_, texture| texture.clone(),
    );
    // A 4-bit channel n samples as n / 15, which an 8-bit target holds as n * 17.
    assert_eq!(seen, [17, 34, 51, 68]);
}

/// Returns a view of a whole one-level 2D texture in `pixel_format` whose reads swizzle as `swizzle` says.
fn swizzled_view<M: Metal>(
    subject: &mut M,
    texture: &M::Texture,
    pixel_format: PixelFormat,
    swizzle: TextureSwizzleChannels,
) -> Result<M::Texture, wcontainers_metal::MakeTextureViewError> {
    subject.make_texture_view(
        texture,
        &TextureViewDescriptor {
            pixel_format,
            texture_type: TextureType::Type2D,
            first_level: u32::MIN,
            level_count: SINGLE,
            first_slice: u32::MIN,
            slice_count: SINGLE,
            swizzle,
        },
    )
}

/// Checks that a view's swizzle moves and replaces the channels its samples give.
///
/// Of an `rgba8Unorm` texture, and of an `abgr4Unorm` one holding Direct3D's `B4G4R4A4` bits (blue in the low four
/// bits, alpha in the high), which the swizzle reads back in their places.
pub fn samples_a_swizzled_view<M: Metal>(mut subject: M) {
    let seen = sampled_texel(
        &mut subject,
        PixelFormat::Rgba8Unorm,
        &[10, 20, 30, 40],
        |subject, texture| {
            swizzled_view(
                subject,
                texture,
                PixelFormat::Rgba8Unorm,
                TextureSwizzleChannels {
                    red: TextureSwizzle::Blue,
                    green: TextureSwizzle::Zero,
                    blue: TextureSwizzle::One,
                    alpha: TextureSwizzle::Red,
                },
            )
            .expect("Metal answers make texture view")
        },
    );
    assert_eq!(seen, [30, 0, 255, 10]);
    // Blue 1, green 2, red 3 and alpha 4, from the low bits up.
    let b4g4r4a4: u16 = 0x4321;
    let seen = sampled_texel(
        &mut subject,
        PixelFormat::Abgr4Unorm,
        &b4g4r4a4.to_le_bytes(),
        |subject, texture| {
            swizzled_view(
                subject,
                texture,
                PixelFormat::Abgr4Unorm,
                TextureSwizzleChannels {
                    red: TextureSwizzle::Green,
                    green: TextureSwizzle::Blue,
                    blue: TextureSwizzle::Alpha,
                    alpha: TextureSwizzle::Red,
                },
            )
            .expect("Metal answers make texture view")
        },
    );
    assert_eq!(seen, [51, 34, 17, 68]);
}

/// Checks that a swizzle is refused on a view of a texture drawn into, and that no view of a swizzled view is made.
///
/// Each channel its own is taken on both textures.
pub fn refuses_a_swizzle_of_a_texture_drawn_into<M: Metal>(mut subject: M) {
    let swizzle = TextureSwizzleChannels {
        red: TextureSwizzle::Alpha,
        ..TextureSwizzleChannels::IDENTITY
    };
    let drawn = target(&mut subject);
    assert_eq!(
        swizzled_view(&mut subject, &drawn, PixelFormat::Rgba8Unorm, swizzle).err(),
        Some(wcontainers_metal::MakeTextureViewError::Incompatible)
    );
    swizzled_view(
        &mut subject,
        &drawn,
        PixelFormat::Rgba8Unorm,
        TextureSwizzleChannels::IDENTITY,
    )
    .expect("Metal answers make texture view");
    let read = subject
        .make_texture(&TextureDescriptor {
            usage: TextureUsage {
                shader_read: true,
                shader_write: false,
                render_target: false,
                pixel_format_view: true,
            },
            ..described(TextureType::Type2D, PixelFormat::Rgba8Unorm)
        })
        .expect("Metal answers make texture");
    let view = swizzled_view(&mut subject, &read, PixelFormat::Rgba8Unorm, swizzle)
        .expect("Metal answers make texture view");
    for again in [swizzle, TextureSwizzleChannels::IDENTITY] {
        assert_eq!(
            swizzled_view(&mut subject, &view, PixelFormat::Rgba8Unorm, again).err(),
            Some(wcontainers_metal::MakeTextureViewError::Incompatible)
        );
    }
}

/// Compiles one entry point of the tests' functions on a thread of Metal's own.
fn function_in_background<M: Metal>(
    subject: &mut M,
    stage: naga::ShaderStage,
    entry_point: &str,
) -> M::Function {
    let (module, info) = programs();
    let vertex_buffers = [VertexBufferLayout {
        buffer_index: VERTICES,
        stride: 8,
        step_function: VertexStepFunction::PerVertex,
        step_rate: 1,
        attributes: vec![VertexAttribute {
            location: u32::MIN,
            offset: u32::MIN,
            format: VertexFormat::Float2,
        }],
    }];
    subject
        .make_function_in_background(&FunctionSource {
            module: &module,
            info: &info,
            stage,
            entry_point,
            resources: SLOTS,
            immediates_buffer_index: IMMEDIATES,
            sizes_buffer_index: SIZES,
            vertex_buffers: &vertex_buffers,
            point_size: false,
        })
        .expect("Metal answers make function in background")
}

/// Checks that functions and pipelines built in the background draw and dispatch as those built at once.
///
/// More pipelines are pending at once than any thread pool runs, each finished after; a pipeline's checks refuse it at
/// once; and a compute pipeline waits for its function's compile.
pub fn builds_functions_and_pipelines_in_the_background<M: Metal>(mut subject: M) {
    let (green, _) = green_and_clear();
    let mut pending = Vec::new();
    for _ in 0..20 {
        let vertex = function_in_background(&mut subject, naga::ShaderStage::Vertex, VERTEX);
        let fragment = function_in_background(&mut subject, naga::ShaderStage::Fragment, FLAT);
        let mut color_attachments = [None; MAX_COLOR_ATTACHMENTS];
        color_attachments[0] = Some(RenderPipelineColorAttachmentDescriptor {
            pixel_format: PixelFormat::Rgba8Unorm,
            blending: None,
            write_mask: ColorWriteMask {
                red: true,
                green: true,
                blue: true,
                alpha: true,
            },
        });
        let descriptor = RenderPipelineDescriptor {
            vertex_function: &vertex,
            fragment_function: Some(&fragment),
            color_attachments,
            depth_attachment_pixel_format: None,
            stencil_attachment_pixel_format: None,
            raster_sample_count: SINGLE,
            alpha_to_coverage_enabled: false,
            rasterization_enabled: true,
            input_primitive_topology: PrimitiveTopologyClass::Triangle,
        };
        pending.push(
            subject
                .make_render_pipeline_state_in_background(&descriptor)
                .expect("Metal answers make render pipeline state in background"),
        );
        // Its fragment function as the vertex function is refused at once.
        assert!(matches!(
            subject.make_render_pipeline_state_in_background(&RenderPipelineDescriptor {
                vertex_function: &fragment,
                ..descriptor
            }),
            Err(MakeRenderPipelineStateError::Invalid(_))
        ));
    }
    for (k, pending) in pending.iter().enumerate() {
        let state = subject
            .finish_render_pipeline_state(pending)
            .expect("Metal answers finish render pipeline state");
        let draw = RenderCommand::DrawPrimitives {
            primitive_type: PrimitiveType::Triangle,
            vertex_start: u32::MIN,
            vertex_count: COVERING_VERTICES,
            instance_count: SINGLE,
            base_instance: u32::MIN,
        };
        assert_eq!(
            drawn_with(&mut subject, &state, draw),
            green,
            "pipeline {k}"
        );
    }
    // Finishing a pipeline again gives it again.
    subject
        .finish_render_pipeline_state(&pending[0])
        .expect("Metal answers finish render pipeline state");
    let words: Vec<u32> = (1..=WORD_COUNT as u32).collect();
    let buffer = filled(&mut subject, &word_bytes(&words));
    let compute = function_in_background(&mut subject, naga::ShaderStage::Compute, DOUBLE);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_compute_commands(
            &command_buffer,
            &[
                ComputeCommand::SetComputePipelineState(state),
                ComputeCommand::SetBuffer {
                    buffer: buffer.clone(),
                    offset: u64::MIN,
                    length: WORDS_BYTES,
                    index: WORDS_SLOT,
                },
                ComputeCommand::DispatchThreadgroups(Size {
                    width: 2,
                    height: SINGLE,
                    depth: SINGLE,
                }),
            ],
        )
        .expect("Metal answers encode compute commands");
    finish(&mut subject, &command_buffer);
    let mut read = vec![u8::MIN; WORDS_BYTES as usize];
    subject
        .read_contents(&buffer, u64::MIN, &mut read)
        .expect("Metal answers read contents");
    let doubled: Vec<u32> = words.iter().map(|w| w * 2).collect();
    assert_eq!(read, word_bytes(&doubled));
}

/// Checks that a compute function doubles every word of a storage buffer.
pub fn dispatches_a_compute_function<M: Metal>(mut subject: M) {
    let words: Vec<u32> = (1..=WORD_COUNT as u32).collect();
    let buffer = filled(
        &mut subject,
        &words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let compute = function(&mut subject, naga::ShaderStage::Compute, DOUBLE);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_compute_commands(
            &command_buffer,
            &[
                ComputeCommand::SetComputePipelineState(state),
                ComputeCommand::SetBuffer {
                    buffer: buffer.clone(),
                    offset: u64::MIN,
                    length: WORDS_BYTES,
                    index: WORDS_SLOT,
                },
                ComputeCommand::DispatchThreadgroups(Size {
                    width: 2,
                    height: SINGLE,
                    depth: SINGLE,
                }),
            ],
        )
        .expect("Metal answers encode compute commands");
    finish(&mut subject, &command_buffer);
    let mut read = vec![u8::MIN; 32];
    subject
        .read_contents(&buffer, u64::MIN, &mut read)
        .expect("Metal answers read contents");
    let doubled: Vec<u8> = words.iter().flat_map(|w| (w * 2).to_le_bytes()).collect();
    assert_eq!(read, doubled);
}

/// Checks that a compute function sums words through a workgroup variable, whose memory the dispatch must be given.
pub fn dispatches_with_workgroup_memory<M: Metal>(mut subject: M) {
    let words: Vec<u32> = (1..=WORD_COUNT as u32).collect();
    let buffer = filled(
        &mut subject,
        &words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let compute = function(&mut subject, naga::ShaderStage::Compute, SUM);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_compute_commands(
            &command_buffer,
            &[
                ComputeCommand::SetComputePipelineState(state),
                ComputeCommand::SetBuffer {
                    buffer: buffer.clone(),
                    offset: u64::MIN,
                    length: WORDS_BYTES,
                    index: WORDS_SLOT,
                },
                ComputeCommand::DispatchThreadgroups(Size {
                    width: SINGLE,
                    height: SINGLE,
                    depth: SINGLE,
                }),
            ],
        )
        .expect("Metal answers encode compute commands");
    finish(&mut subject, &command_buffer);
    let mut read = [u8::MIN; 4];
    subject
        .read_contents(&buffer, 16, &mut read)
        .expect("Metal answers read contents");
    assert_eq!(u32::from_le_bytes(read), 1 + 2 + 3 + 4);
}

/// One more than the longest side of a 3D texture.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const BEYOND_3D_SIDE: u32 = 2049;

/// One more than the most layers of an array texture.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const BEYOND_LAYERS: u32 = 2049;

/// The fewest cubes of a cube array whose faces pass the most layers of an array texture, 2048.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const BEYOND_CUBES: u32 = 342;

/// The first offset past the visibility result buffer a pass counts into, 256 KB.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const BEYOND_VISIBILITY_SPAN: u64 = 262_144;

/// A row pitch longer than a row of the widest texture any Apple GPU makes, in `rgba8Unorm`.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy(from:sourceoffset:sourcebytesperrow:sourcebytesperimage:sourcesize:to:destinationslice:destinationlevel:destinationorigin:options:)
const BEYOND_ROW: u64 = 160_000;

/// An offset of the uniform the tests read, a four-float color aligned to 16 bytes, that is not a multiple of 16.
///
/// https://www.w3.org/TR/WGSL/#alignment-and-size
const MISALIGNED_UNIFORM: u64 = 4;

/// An offset of the words the compute tests read, aligned to 4 bytes, that is not a multiple of 4.
///
/// https://www.w3.org/TR/WGSL/#alignment-and-size
const MISALIGNED_WORDS: u64 = 2;

/// A sample count every Apple GPU takes.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const SAMPLES: u32 = 4;

/// Returns a descriptor of a `TARGET_SIDE` square texture of one level, read and drawn into.
fn described(texture_type: TextureType, pixel_format: PixelFormat) -> TextureDescriptor {
    let multisampled = texture_type == TextureType::Type2DMultisample;
    TextureDescriptor {
        texture_type,
        pixel_format,
        width: TARGET_SIDE,
        height: TARGET_SIDE,
        depth: SINGLE,
        mipmap_level_count: SINGLE,
        sample_count: if multisampled { SAMPLES } else { 1 },
        array_length: SINGLE,
        usage: TextureUsage {
            shader_read: true,
            shader_write: false,
            render_target: true,
            pixel_format_view: false,
        },
    }
}

/// Checks that a texture of `descriptor` is refused.
fn refuses_texture<M: Metal>(subject: &mut M, descriptor: TextureDescriptor) {
    assert!(matches!(
        subject.make_texture(&descriptor),
        Err(MakeTextureError::Invalid(_))
    ));
}

/// Makes a texture of `descriptor`.
fn made<M: Metal>(subject: &mut M, descriptor: TextureDescriptor) -> M::Texture {
    subject
        .make_texture(&descriptor)
        .expect("Metal answers make texture")
}

/// Returns a pipeline descriptor drawing `fragment` into a color of `format`, blended or not, with depth and stencil.
fn pipeline_descriptor<'a, M: Metal>(
    vertex: &'a M::Function,
    fragment: &'a M::Function,
    color: Option<(PixelFormat, Option<Blending>)>,
    depth: Option<PixelFormat>,
    stencil: Option<PixelFormat>,
) -> RenderPipelineDescriptor<'a, M> {
    let mut color_attachments = [None; MAX_COLOR_ATTACHMENTS];
    color_attachments[0] =
        color.map(
            |(pixel_format, blending)| RenderPipelineColorAttachmentDescriptor {
                pixel_format,
                blending,
                write_mask: ColorWriteMask {
                    red: true,
                    green: true,
                    blue: true,
                    alpha: true,
                },
            },
        );
    RenderPipelineDescriptor {
        vertex_function: vertex,
        fragment_function: Some(fragment),
        color_attachments,
        depth_attachment_pixel_format: depth,
        stencil_attachment_pixel_format: stencil,
        raster_sample_count: SINGLE,
        alpha_to_coverage_enabled: false,
        rasterization_enabled: true,
        input_primitive_topology: PrimitiveTopologyClass::Triangle,
    }
}

/// Checks that a pipeline is refused.
fn refuses_pipeline<M: Metal>(
    subject: &mut M,
    fragment: &str,
    color: Option<(PixelFormat, Option<Blending>)>,
    depth: Option<PixelFormat>,
    stencil: Option<PixelFormat>,
) {
    let vertex = function(subject, naga::ShaderStage::Vertex, VERTEX);
    let fragment = function(subject, naga::ShaderStage::Fragment, fragment);
    let descriptor = pipeline_descriptor::<M>(&vertex, &fragment, color, depth, stencil);
    assert!(matches!(
        subject.make_render_pipeline_state(&descriptor),
        Err(MakeRenderPipelineStateError::Invalid(_))
    ));
}

/// Returns a buffer holding the covering triangle.
fn covering<M: Metal>(subject: &mut M) -> M::Buffer {
    let bytes: Vec<u8> = COVERING
        .as_flattened()
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    filled(subject, &bytes)
}

/// Checks that the covering draw in the uniform's color, with `commands` after its bindings, is refused in `pass`.
fn refuses_draw<M: Metal>(
    subject: &mut M,
    pass: &RenderPassDescriptor<'_, M>,
    state: &M::RenderPipelineState,
    commands: Vec<RenderCommand<M>>,
) {
    let depth_stencil = no_depth(subject);
    let vertices = covering(subject);
    let color = filled(subject, &[u8::MIN; COLOR_BYTES as usize]);
    let mut bindings = vec![RenderCommand::SetFragmentBuffer {
        buffer: color,
        offset: u64::MIN,
        length: COLOR_BYTES,
        index: u32::MIN,
    }];
    bindings.extend(commands);
    let commands = covering_draw(state, &vertices, &depth_stencil, bindings);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    assert!(
        subject
            .encode_render_commands(&command_buffer, pass, &commands)
            .is_err()
    );
}

/// Returns a stencil test that counts what passes.
fn counting_stencil() -> StencilDescriptor {
    StencilDescriptor {
        stencil_compare_function: CompareFunction::Always,
        stencil_failure_operation: StencilOperation::Keep,
        depth_failure_operation: StencilOperation::Keep,
        depth_stencil_pass_operation: StencilOperation::IncrementClamp,
        read_mask: u32::from(u8::MAX),
        write_mask: u32::from(u8::MAX),
    }
}

/// Checks that one copy or fill is refused.
fn refuses_blit<M: Metal>(subject: &mut M, command: BlitCommand<M>) {
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    assert!(matches!(
        subject.encode_blit_commands(&command_buffer, &[command]),
        Err(EncodeBlitCommandsError::Invalid(_))
    ));
}

/// Checks that a texture wider than any Apple GPU makes is refused, where Metal stops the process.
pub fn refuses_a_texture_wider_than_any_gpu_makes<M: Metal>(mut subject: M) {
    let mut descriptor = described(TextureType::Type2D, PixelFormat::R8Unorm);
    descriptor.width = MAX_TEXTURE_SIDE + 1;
    refuses_texture(&mut subject, descriptor);
    let mut descriptor = described(TextureType::TypeCube, PixelFormat::R8Unorm);
    descriptor.width = MAX_TEXTURE_SIDE + 1;
    descriptor.height = MAX_TEXTURE_SIDE + 1;
    refuses_texture(&mut subject, descriptor);
}

/// Checks that a 3D texture deeper than 2048 is refused, where Metal stops the process.
pub fn refuses_a_3d_texture_deeper_than_the_most<M: Metal>(mut subject: M) {
    let mut descriptor = described(TextureType::Type3D, PixelFormat::R8Unorm);
    descriptor.depth = BEYOND_3D_SIDE;
    refuses_texture(&mut subject, descriptor);
}

/// Checks that an array of more layers than 2048 is refused, where Metal stops the process.
pub fn refuses_more_layers_than_the_most<M: Metal>(mut subject: M) {
    let mut descriptor = described(TextureType::Type2DArray, PixelFormat::R8Unorm);
    descriptor.array_length = BEYOND_LAYERS;
    refuses_texture(&mut subject, descriptor);
    let mut descriptor = described(TextureType::TypeCubeArray, PixelFormat::R8Unorm);
    descriptor.array_length = BEYOND_CUBES;
    refuses_texture(&mut subject, descriptor);
}

/// Checks that a texture in the stencil view format is refused, which Metal's validation stops.
pub fn refuses_a_texture_in_a_view_only_format<M: Metal>(mut subject: M) {
    refuses_texture(
        &mut subject,
        described(TextureType::Type2D, PixelFormat::X32Stencil8),
    );
}

/// Checks that a stencil view of a texture not made for views is refused, which Metal's validation stops.
pub fn refuses_a_stencil_view_of_a_texture_not_made_for_views<M: Metal>(mut subject: M) {
    let texture = made(
        &mut subject,
        described(TextureType::Type2D, PixelFormat::Depth32FloatStencil8),
    );
    assert!(
        subject
            .make_texture_view(
                &texture,
                &TextureViewDescriptor {
                    pixel_format: PixelFormat::X32Stencil8,
                    texture_type: TextureType::Type2D,
                    first_level: u32::MIN,
                    level_count: SINGLE,
                    first_slice: u32::MIN,
                    slice_count: SINGLE,
                    swizzle: TextureSwizzleChannels::IDENTITY,
                },
            )
            .is_err()
    );
}

/// Checks that a pipeline drawing into a compressed format is refused, where Metal stops the process.
pub fn refuses_a_pipeline_drawing_into_a_compressed_format<M: Metal>(mut subject: M) {
    refuses_pipeline(
        &mut subject,
        FLAT,
        Some((PixelFormat::Bc1Rgba, None)),
        None,
        None,
    );
}

/// Checks that a pipeline blending into an integer format is refused, where Metal stops the process.
pub fn refuses_a_pipeline_blending_an_integer_format<M: Metal>(mut subject: M) {
    let blending = Blending {
        rgb_blend_operation: BlendOperation::Add,
        alpha_blend_operation: BlendOperation::Add,
        source_rgb_blend_factor: BlendFactor::One,
        destination_rgb_blend_factor: BlendFactor::One,
        source_alpha_blend_factor: BlendFactor::One,
        destination_alpha_blend_factor: BlendFactor::One,
    };
    refuses_pipeline(
        &mut subject,
        WHOLE,
        Some((PixelFormat::Rgba8Uint, Some(blending))),
        None,
        None,
    );
}

/// Checks that a pipeline of a combined depth and stencil format beside another is refused, where Metal stops.
pub fn refuses_a_pipeline_of_mismatched_depth_and_stencil<M: Metal>(mut subject: M) {
    refuses_pipeline(
        &mut subject,
        FLAT,
        Some((PixelFormat::Rgba8Unorm, None)),
        Some(PixelFormat::Depth32FloatStencil8),
        Some(PixelFormat::Stencil8),
    );
}

/// Checks that a pass drawing into a compressed texture is refused, which Metal's validation stops.
pub fn refuses_a_pass_drawing_into_a_compressed_texture<M: Metal>(mut subject: M) {
    let texture = made(
        &mut subject,
        described(TextureType::Type2D, PixelFormat::Bc1Rgba),
    );
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    assert!(
        subject
            .encode_render_commands(&command_buffer, &pass(&texture), &[])
            .is_err()
    );
}

/// Checks that a depth clear outside zero to one is refused, which Metal's validation stops.
pub fn refuses_a_depth_clear_outside_zero_to_one<M: Metal>(mut subject: M) {
    let color = target(&mut subject);
    let depth = made(
        &mut subject,
        described(TextureType::Type2D, PixelFormat::Depth32Float),
    );
    let mut descriptor = pass(&color);
    descriptor.depth_attachment = Some(RenderPassDepthStencilAttachment {
        texture: &depth,
        level: u32::MIN,
        slice: u32::MIN,
        load_action: LoadAction::Clear,
        store_action: StoreAction::Store,
        clear_value: 2.0,
    });
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    assert!(
        subject
            .encode_render_commands(&command_buffer, &descriptor, &[])
            .is_err()
    );
}

/// Checks that resolves of an integer format, and into another format, are refused, which Metal's validation stops.
pub fn refuses_a_resolve_metal_does_not_make<M: Metal>(mut subject: M) {
    for (from, into) in [
        (PixelFormat::Rgba8Uint, PixelFormat::Rgba8Uint),
        (PixelFormat::R32Float, PixelFormat::Rg16Unorm),
    ] {
        let source = made(
            &mut subject,
            described(TextureType::Type2DMultisample, from),
        );
        let resolved = made(&mut subject, described(TextureType::Type2D, into));
        let mut descriptor = pass(&source);
        if let Some(attachment) = descriptor.color_attachments[0].as_mut() {
            attachment.resolve = Some(ResolveTarget {
                texture: &resolved,
                level: u32::MIN,
                slice: u32::MIN,
                depth_plane: u32::MIN,
            });
        }
        descriptor.default_raster_sample_count = SAMPLES;
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        assert!(
            subject
                .encode_render_commands(&command_buffer, &descriptor, &[])
                .is_err()
        );
    }
}

/// Checks that a viewport that is not finite is refused, which Metal's validation stops.
pub fn refuses_a_viewport_that_is_not_finite<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    let viewport = Viewport {
        origin_x: f64::NAN,
        ..TARGET_VIEWPORT
    };
    refuses_draw(
        &mut subject,
        &pass(&texture),
        &state,
        vec![RenderCommand::SetViewports(vec![viewport])],
    );
}

/// Checks that a uniform at an offset not a multiple of its alignment is refused, which Metal's validation stops.
pub fn refuses_a_uniform_at_a_misaligned_offset<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    let color = filled(&mut subject, &[u8::MIN; (COLOR_BYTES * 2) as usize]);
    let bound = RenderCommand::SetFragmentBuffer {
        buffer: color,
        offset: MISALIGNED_UNIFORM,
        length: COLOR_BYTES,
        index: u32::MIN,
    };
    refuses_draw(&mut subject, &pass(&texture), &state, vec![bound]);
}

/// Checks that draws of no instances and of no indices are refused, which Metal's validation stops.
pub fn refuses_draws_of_no_instances_or_indices<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    let indices = filled(&mut subject, &[u8::MIN; COVERING_BYTES as usize]);
    for draw in [
        RenderCommand::DrawPrimitives {
            primitive_type: PrimitiveType::Triangle,
            vertex_start: u32::MIN,
            vertex_count: COVERING_VERTICES,
            instance_count: u32::MIN,
            base_instance: u32::MIN,
        },
        RenderCommand::DrawIndexedPrimitives {
            primitive_type: PrimitiveType::Triangle,
            index_count: u32::MIN,
            index_type: IndexType::UInt16,
            index_buffer: indices,
            index_buffer_offset: u64::MIN,
            instance_count: SINGLE,
            base_vertex: i32::default(),
            base_instance: u32::MIN,
        },
    ] {
        refuses_draw(&mut subject, &pass(&texture), &state, vec![draw]);
    }
}

/// Checks that a texture not made to be read by functions is refused to one, which Metal's validation stops.
pub fn refuses_a_texture_not_made_to_be_read<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, SAMPLED);
    let drawn = target(&mut subject);
    let mut descriptor = described(TextureType::Type2D, PixelFormat::Rgba8Unorm);
    descriptor.usage.shader_read = false;
    let unread = made(&mut subject, descriptor);
    let sampler = subject
        .make_sampler_state(&SamplerDescriptor {
            min_filter: SamplerMinMagFilter::Nearest,
            mag_filter: SamplerMinMagFilter::Nearest,
            mip_filter: SamplerMipFilter::NotMipmapped,
            s_address_mode: SamplerAddressMode::ClampToEdge,
            t_address_mode: SamplerAddressMode::ClampToEdge,
            r_address_mode: SamplerAddressMode::ClampToEdge,
            border_color: SamplerBorderColor::TransparentBlack,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            max_anisotropy: SINGLE,
            compare_function: None,
        })
        .expect("Metal answers make sampler state");
    let bound = vec![
        RenderCommand::SetFragmentTexture {
            texture: unread,
            index: u32::MIN,
        },
        RenderCommand::SetFragmentSamplerState {
            sampler,
            index: u32::MIN,
        },
    ];
    refuses_draw(&mut subject, &pass(&drawn), &state, bound);
}

/// Checks that counting past the visibility result buffer's span is refused, which Metal's validation stops.
pub fn refuses_a_visibility_offset_past_the_span<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    let results = subject
        .make_buffer(BEYOND_VISIBILITY_SPAN * 2)
        .expect("Metal answers make buffer");
    let mut descriptor = pass(&texture);
    descriptor.visibility_result_buffer = Some(&results);
    let counting = RenderCommand::SetVisibilityResultMode {
        mode: VisibilityResultMode::Counting,
        offset: BEYOND_VISIBILITY_SPAN,
    };
    refuses_draw(&mut subject, &descriptor, &state, vec![counting]);
}

/// Checks that depth and stencil tests in a pass without those attachments are refused, which Metal's validation stops.
pub fn refuses_depth_and_stencil_tests_without_attachments<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    for descriptor in [
        DepthStencilDescriptor {
            depth_compare_function: CompareFunction::Less,
            depth_write_enabled: false,
            front_face_stencil: None,
            back_face_stencil: None,
        },
        DepthStencilDescriptor {
            depth_compare_function: CompareFunction::Always,
            depth_write_enabled: false,
            front_face_stencil: Some(counting_stencil()),
            back_face_stencil: Some(counting_stencil()),
        },
    ] {
        let tests = subject
            .make_depth_stencil_state(&descriptor)
            .expect("Metal answers make depth stencil state");
        refuses_draw(
            &mut subject,
            &pass(&texture),
            &state,
            vec![RenderCommand::SetDepthStencilState(tests)],
        );
    }
}

/// Draws the covering triangle in green with depth test bounds `bounds` over a stored depth of `stored`.
///
/// The target's `depth32Float` depth is cleared to `stored` and nothing else is tested; returns the target's texels or
/// the encode's refusal.
fn drawn_within_bounds<M: Metal>(
    subject: &mut M,
    stored: f64,
    bounds: (f32, f32),
) -> Result<Vec<u8>, EncodeRenderCommandsError> {
    let vertex = function(subject, naga::ShaderStage::Vertex, VERTEX);
    let fragment = function(subject, naga::ShaderStage::Fragment, FLAT);
    let state = subject
        .make_render_pipeline_state(&pipeline_descriptor(
            &vertex,
            &fragment,
            Some((PixelFormat::Rgba8Unorm, None)),
            Some(PixelFormat::Depth32Float),
            None,
        ))
        .expect("Metal answers make render pipeline state");
    let depth = made(
        subject,
        described(TextureType::Type2D, PixelFormat::Depth32Float),
    );
    let drawn = target(subject);
    let mut descriptor = pass(&drawn);
    descriptor.depth_attachment = Some(RenderPassDepthStencilAttachment {
        texture: &depth,
        level: u32::MIN,
        slice: u32::MIN,
        load_action: LoadAction::Clear,
        store_action: StoreAction::Store,
        clear_value: stored,
    });
    let (min_bound, max_bound) = bounds;
    let mut commands = green_draw(
        subject,
        &state,
        RenderCommand::SetDepthTestBounds {
            min_bound,
            max_bound,
        },
    );
    commands.push(RenderCommand::DrawPrimitives {
        primitive_type: PrimitiveType::Triangle,
        vertex_start: u32::MIN,
        vertex_count: COVERING_VERTICES,
        instance_count: SINGLE,
        base_instance: u32::MIN,
    });
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject.encode_render_commands(&command_buffer, &descriptor, &commands)?;
    finish(subject, &command_buffer);
    Ok(read_texture(subject, &drawn))
}

/// A depth halfway between the nearest and the farthest.
///
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/setdepthtestbounds(_:)
const MIDDLE_DEPTH: f64 = 0.5;

/// Stored depths, the bounds they are tested against, and whether the fragment over each is drawn.
///
/// A fragment is drawn where the depth stored under it lies within the bounds, both ends included.
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/setdepthtestbounds(_:)
const DEPTH_BOUNDS_CASES: [(f64, (f32, f32), bool); 5] = [
    (0.75, (0.0, 0.5), false),
    (0.75, (0.5, 1.0), true),
    (0.5, (0.5, 0.5), true),
    (0.25, (0.5, 1.0), false),
    (0.25, (0.0, 1.0), true),
];

/// Depth bounds that are refused: outside zero to one, out of order, or not numbers.
///
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/setdepthtestbounds(_:)
const REFUSED_DEPTH_BOUNDS: [(f32, f32); 5] = [
    (-0.25, 0.5),
    (0.25, 1.5),
    (0.75, 0.25),
    (f32::NAN, 1.0),
    (0.0, f32::INFINITY),
];

/// Checks that depth test bounds discard the fragments whose stored depth lies outside them, bounds included.
///
/// Where Metal has them; and that they are refused where it lacks them, outside zero to one, out of order, not
/// numbers, or in a pass with no depth attachment.
pub fn discards_fragments_whose_stored_depth_is_outside_the_bounds<M: Metal>(mut subject: M) {
    let (green, clear) = green_and_clear();
    if !subject.supports_depth_test_bounds() {
        for (_, bounds, _) in DEPTH_BOUNDS_CASES {
            assert!(drawn_within_bounds(&mut subject, MIDDLE_DEPTH, bounds).is_err());
        }
        for bounds in REFUSED_DEPTH_BOUNDS {
            assert!(drawn_within_bounds(&mut subject, MIDDLE_DEPTH, bounds).is_err());
        }
        return;
    }
    for (stored, bounds, draws) in DEPTH_BOUNDS_CASES {
        assert_eq!(
            &drawn_within_bounds(&mut subject, stored, bounds).expect("bounds taken"),
            if draws { &green } else { &clear },
            "stored depth {stored} within {bounds:?}"
        );
    }
    for bounds in REFUSED_DEPTH_BOUNDS {
        assert!(
            matches!(
                drawn_within_bounds(&mut subject, MIDDLE_DEPTH, bounds),
                Err(EncodeRenderCommandsError::Invalid(_))
            ),
            "bounds {bounds:?}"
        );
    }
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    refuses_draw(
        &mut subject,
        &pass(&texture),
        &state,
        vec![RenderCommand::SetDepthTestBounds {
            min_bound: 0.25,
            max_bound: 0.75,
        }],
    );
}

/// Checks that an empty dispatch and words at a misaligned offset are refused, which Metal's validation stops.
pub fn refuses_empty_dispatches_and_misaligned_words<M: Metal>(mut subject: M) {
    let words = filled(&mut subject, &[u8::MIN; (WORDS_BYTES * 2) as usize]);
    let compute = function(&mut subject, naga::ShaderStage::Compute, DOUBLE);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    let one = Size {
        width: SINGLE,
        height: SINGLE,
        depth: SINGLE,
    };
    let none = Size {
        width: u32::MIN,
        ..one
    };
    for (offset, groups) in [(0, none), (MISALIGNED_WORDS, one)] {
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        let commands = [
            ComputeCommand::SetComputePipelineState(state.clone()),
            ComputeCommand::SetBuffer {
                buffer: words.clone(),
                offset,
                length: WORDS_BYTES,
                index: WORDS_SLOT,
            },
            ComputeCommand::DispatchThreadgroups(groups),
        ];
        assert!(matches!(
            subject.encode_compute_commands(&command_buffer, &commands),
            Err(EncodeComputeCommandsError::Invalid(_) | EncodeComputeCommandsError::Unbound(_))
        ));
    }
}

/// Checks that buffer copies and fills of no bytes or not in whole words are refused, as Metal on macOS asks.
pub fn refuses_buffer_copies_and_fills_metal_does_not_make<M: Metal>(mut subject: M) {
    let buffer = filled(&mut subject, &pattern(COVERING_BYTES as usize));
    let copy = |source_offset, destination_offset, size| BlitCommand::CopyFromBufferToBuffer {
        source_buffer: buffer.clone(),
        source_offset,
        destination_buffer: buffer.clone(),
        destination_offset,
        size,
    };
    let fill = |offset, length| BlitCommand::FillBuffer {
        buffer: buffer.clone(),
        offset,
        length,
        value: FILL_BYTE,
    };
    for command in [
        copy(u64::MIN, RGBA8_BYTES * 2, u64::MIN),
        copy(1, RGBA8_BYTES * 2, RGBA8_BYTES),
        copy(u64::MIN, RGBA8_BYTES * 2, 3),
        fill(u64::MIN, u64::MIN),
        fill(1, RGBA8_BYTES),
        fill(u64::MIN, 3),
    ] {
        refuses_blit(&mut subject, command);
    }
}

/// Checks that image pitches shorter than an image and row pitches past the longest row are refused, as Metal asks.
pub fn refuses_copy_pitches_metal_does_not_take<M: Metal>(mut subject: M) {
    let texture = target(&mut subject);
    let buffer = subject
        .make_buffer(BEYOND_ROW * 2)
        .expect("Metal answers make buffer");
    let row = u64::from(TARGET_SIDE) * RGBA8_BYTES;
    let copy = |bytes_per_row, bytes_per_image, height| BlitCommand::CopyFromBufferToTexture {
        source_buffer: buffer.clone(),
        source_offset: u64::MIN,
        source_bytes_per_row: bytes_per_row,
        source_bytes_per_image: bytes_per_image,
        source_size: Size {
            width: TARGET_SIDE,
            height,
            depth: SINGLE,
        },
        destination_texture: texture.clone(),
        destination_slice: u32::MIN,
        destination_level: u32::MIN,
        destination_origin: Origin::default(),
        options: BlitOption::None,
    };
    refuses_blit(&mut subject, copy(row, RGBA8_BYTES, TARGET_SIDE));
    refuses_blit(&mut subject, copy(BEYOND_ROW, u64::MIN, 1));
    // A 3D texture's rows are as long as its own longest side at most, shorter than a 2D texture's (MEASURED: Metal's
    // validation stops 24000 bytes past 16384, found by the fuzzer).
    let volume = made(
        &mut subject,
        described(TextureType::Type3D, PixelFormat::Rgba8Unorm),
    );
    refuses_blit(
        &mut subject,
        BlitCommand::CopyFromBufferToTexture {
            source_buffer: buffer.clone(),
            source_offset: u64::MIN,
            source_bytes_per_row: u64::from(BEYOND_3D_SIDE) * RGBA8_BYTES,
            source_bytes_per_image: u64::MIN,
            source_size: Size {
                width: TARGET_SIDE,
                height: 1,
                depth: SINGLE,
            },
            destination_texture: volume,
            destination_slice: u32::MIN,
            destination_level: u32::MIN,
            destination_origin: Origin::default(),
            options: BlitOption::None,
        },
    );
}

/// Checks that a copy between an sRGB texture and one of its linear twin is refused, which Metal's validation stops.
pub fn refuses_a_copy_between_texture_formats_that_differ<M: Metal>(mut subject: M) {
    let linear = target(&mut subject);
    let srgb = made(
        &mut subject,
        described(TextureType::Type2D, PixelFormat::Rgba8UnormSrgb),
    );
    refuses_blit(
        &mut subject,
        BlitCommand::CopyFromTextureToTexture {
            source_texture: linear,
            source_slice: u32::MIN,
            source_level: u32::MIN,
            source_origin: Origin::default(),
            source_size: Size {
                width: TARGET_SIDE,
                height: TARGET_SIDE,
                depth: SINGLE,
            },
            destination_texture: srgb,
            destination_slice: u32::MIN,
            destination_level: u32::MIN,
            destination_origin: Origin::default(),
        },
    );
}

/// Checks that counting twice at one offset in a pass is refused, which Metal's validation stops.
pub fn refuses_counting_twice_at_one_offset<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    let results = subject
        .make_buffer(VISIBILITY_RESULT_BYTES)
        .expect("Metal answers make buffer");
    let mut descriptor = pass(&texture);
    descriptor.visibility_result_buffer = Some(&results);
    let count = |mode| RenderCommand::SetVisibilityResultMode {
        mode,
        offset: u64::MIN,
    };
    let commands = vec![
        count(VisibilityResultMode::Counting),
        count(VisibilityResultMode::Disabled),
        count(VisibilityResultMode::Counting),
    ];
    refuses_draw(&mut subject, &descriptor, &state, commands);
}

/// Checks that occlusion counts add up across passes: one sample for each covered pixel in each.
pub fn adds_occlusion_counts_across_passes<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let depth_stencil = no_depth(&mut subject);
    let vertices = covering(&mut subject);
    let color = filled(&mut subject, &[u8::MIN; COLOR_BYTES as usize]);
    let texture = target(&mut subject);
    let results = filled(&mut subject, &[u8::MIN; VISIBILITY_RESULT_BYTES as usize]);
    let mut descriptor = pass(&texture);
    descriptor.visibility_result_buffer = Some(&results);
    let bindings = vec![
        RenderCommand::SetFragmentBuffer {
            buffer: color,
            offset: u64::MIN,
            length: COLOR_BYTES,
            index: u32::MIN,
        },
        RenderCommand::SetVisibilityResultMode {
            mode: VisibilityResultMode::Counting,
            offset: u64::MIN,
        },
    ];
    let commands = covering_draw(&state, &vertices, &depth_stencil, bindings);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    for _ in 0..2 {
        subject
            .encode_render_commands(&command_buffer, &descriptor, &commands)
            .expect("Metal answers encode render commands");
    }
    finish(&mut subject, &command_buffer);
    let mut read = [u8::MIN; VISIBILITY_RESULT_BYTES as usize];
    subject
        .read_contents(&results, u64::MIN, &mut read)
        .expect("Metal answers read contents");
    assert_eq!(
        u64::from_le_bytes(read),
        u64::from(2 * TARGET_SIDE * TARGET_SIDE)
    );
}

/// Checks that mipmaps of a texture of one level are refused, which Metal's documentation forbids.
pub fn refuses_mipmaps_of_one_level<M: Metal>(mut subject: M) {
    let texture = target(&mut subject);
    refuses_blit(&mut subject, BlitCommand::GenerateMipmaps { texture });
}

/// Returns a nearest sampler with the three address modes.
fn sampler_of<M: Metal>(
    subject: &mut M,
    modes: [SamplerAddressMode; 3],
) -> Result<M::SamplerState, MakeSamplerStateError> {
    subject.make_sampler_state(&SamplerDescriptor {
        min_filter: SamplerMinMagFilter::Nearest,
        mag_filter: SamplerMinMagFilter::Nearest,
        mip_filter: SamplerMipFilter::NotMipmapped,
        s_address_mode: modes[0],
        t_address_mode: modes[1],
        r_address_mode: modes[2],
        border_color: SamplerBorderColor::TransparentBlack,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        max_anisotropy: SINGLE,
        compare_function: None,
    })
}

/// Checks that dispatching the doubling function after `bindings` is refused.
fn refuses_dispatch<M: Metal>(subject: &mut M, bindings: Vec<ComputeCommand<M>>) {
    let compute = function(subject, naga::ShaderStage::Compute, DOUBLE);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    let mut commands = vec![ComputeCommand::SetComputePipelineState(state)];
    commands.extend(bindings);
    commands.push(ComputeCommand::DispatchThreadgroups(Size {
        width: 2,
        height: SINGLE,
        depth: SINGLE,
    }));
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    assert!(
        subject
            .encode_compute_commands(&command_buffer, &commands)
            .is_err()
    );
}

/// The image pitches whose sums and products with a second image wrap past the largest `u64`, or come near it.
///
/// https://doc.rust-lang.org/std/primitive.u64.html#associatedconstant.MAX
const WRAPPING_PITCHES: [u64; 5] = [
    u64::MAX,
    u64::MAX - 63,
    1 << 63,
    (1 << 63) + 64,
    u64::MAX / 2 + 1,
];

/// The offsets whose sum with a small length wraps past the largest `u64`.
///
/// https://doc.rust-lang.org/std/primitive.u64.html#associatedconstant.MAX
const WRAPPING_OFFSETS: [u64; 3] = [u64::MAX, u64::MAX - 3, u64::MAX - 7];

/// Checks that every range, place and count whose end wraps past the largest integer is refused, at each such value.
///
/// A 3D copy whose image pitch made its length wrap once passed the range check and wrote into another buffer.
pub fn refuses_every_range_whose_end_wraps<M: Metal>(mut subject: M) {
    let mut descriptor = described(TextureType::Type3D, PixelFormat::Rgba8Unorm);
    descriptor.depth = 2;
    descriptor.usage.render_target = false;
    let volume = made(&mut subject, descriptor);
    let buffer = filled(&mut subject, &pattern(256));
    let row = u64::from(TARGET_SIDE) * RGBA8_BYTES;
    let deep = Size {
        width: TARGET_SIDE,
        height: TARGET_SIDE,
        depth: 2,
    };
    for pitch in WRAPPING_PITCHES {
        let out = BlitCommand::CopyFromTextureToBuffer {
            source_texture: volume.clone(),
            source_slice: u32::MIN,
            source_level: u32::MIN,
            source_origin: Origin::default(),
            source_size: deep,
            destination_buffer: buffer.clone(),
            destination_offset: u64::MIN,
            destination_bytes_per_row: row,
            destination_bytes_per_image: pitch,
            options: BlitOption::None,
        };
        let into = BlitCommand::CopyFromBufferToTexture {
            source_buffer: buffer.clone(),
            source_offset: u64::MIN,
            source_bytes_per_row: row,
            source_bytes_per_image: pitch,
            source_size: deep,
            destination_texture: volume.clone(),
            destination_slice: u32::MIN,
            destination_level: u32::MIN,
            destination_origin: Origin::default(),
            options: BlitOption::None,
        };
        for command in [out, into] {
            let command_buffer = subject
                .make_command_buffer()
                .expect("Metal answers make command buffer");
            assert!(
                subject
                    .encode_blit_commands(&command_buffer, &[command])
                    .is_err(),
                "an image pitch of {pitch}"
            );
        }
    }
    for offset in WRAPPING_OFFSETS {
        let word = RGBA8_BYTES;
        let commands = [
            BlitCommand::CopyFromBufferToBuffer {
                source_buffer: buffer.clone(),
                source_offset: offset,
                destination_buffer: buffer.clone(),
                destination_offset: u64::MIN,
                size: word,
            },
            BlitCommand::CopyFromBufferToBuffer {
                source_buffer: buffer.clone(),
                source_offset: u64::MIN,
                destination_buffer: buffer.clone(),
                destination_offset: offset,
                size: word,
            },
            BlitCommand::FillBuffer {
                buffer: buffer.clone(),
                offset,
                length: word,
                value: FILL_BYTE,
            },
        ];
        for command in commands {
            let command_buffer = subject
                .make_command_buffer()
                .expect("Metal answers make command buffer");
            assert!(
                subject
                    .encode_blit_commands(&command_buffer, &[command])
                    .is_err(),
                "an offset of {offset}"
            );
        }
        assert!(
            subject
                .write_contents(&buffer, offset, &[u8::MIN; 8])
                .is_err()
        );
        assert!(
            subject
                .read_contents(&buffer, offset, &mut [u8::MIN; 8])
                .is_err()
        );
        refuses_dispatch(
            &mut subject,
            vec![ComputeCommand::SetBuffer {
                buffer: buffer.clone(),
                offset,
                length: WORDS_BYTES,
                index: WORDS_SLOT,
            }],
        );
    }
    let texture = target(&mut subject);
    for (first_level, first_slice) in [(u32::MAX, u32::MIN), (u32::MIN, u32::MAX)] {
        assert!(
            subject
                .make_texture_view(
                    &texture,
                    &TextureViewDescriptor {
                        pixel_format: PixelFormat::Rgba8Unorm,
                        texture_type: TextureType::Type2D,
                        first_level,
                        level_count: SINGLE,
                        first_slice,
                        slice_count: SINGLE,
                        swizzle: TextureSwizzleChannels::IDENTITY,
                    },
                )
                .is_err()
        );
    }
    let one = Size {
        width: SINGLE,
        height: SINGLE,
        depth: SINGLE,
    };
    for origin in [
        Origin {
            x: u32::MAX,
            ..Origin::default()
        },
        Origin {
            y: u32::MAX,
            ..Origin::default()
        },
        Origin {
            z: u32::MAX,
            ..Origin::default()
        },
    ] {
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        let copy = BlitCommand::CopyFromTextureToTexture {
            source_texture: texture.clone(),
            source_slice: u32::MIN,
            source_level: u32::MIN,
            source_origin: origin,
            source_size: one,
            destination_texture: target(&mut subject),
            destination_slice: u32::MIN,
            destination_level: u32::MIN,
            destination_origin: Origin::default(),
        };
        assert!(
            subject
                .encode_blit_commands(&command_buffer, &[copy])
                .is_err()
        );
    }
    let state = pipeline(&mut subject, FLAT);
    let drawn = target(&mut subject);
    let results = filled(&mut subject, &[u8::MIN; VISIBILITY_RESULT_BYTES as usize]);
    let mut counted = pass(&drawn);
    counted.visibility_result_buffer = Some(&results);
    let indices = filled(&mut subject, &pattern(64));
    let draws = [
        RenderCommand::SetVertexBuffer {
            buffer: buffer.clone(),
            offset: u64::MAX - 3,
            length: RGBA8_BYTES,
            index: VERTICES,
        },
        RenderCommand::SetScissorRects(vec![ScissorRect {
            x: u32::MAX,
            y: u32::MIN,
            width: SINGLE,
            height: SINGLE,
        }]),
        RenderCommand::DrawPrimitives {
            primitive_type: PrimitiveType::Triangle,
            vertex_start: u32::MAX,
            vertex_count: COVERING_VERTICES,
            instance_count: SINGLE,
            base_instance: u32::MIN,
        },
        RenderCommand::DrawPrimitives {
            primitive_type: PrimitiveType::Triangle,
            vertex_start: u32::MIN,
            vertex_count: COVERING_VERTICES,
            instance_count: SINGLE,
            base_instance: u32::MAX,
        },
        RenderCommand::DrawIndexedPrimitives {
            primitive_type: PrimitiveType::Triangle,
            index_count: u32::MAX,
            index_type: IndexType::UInt32,
            index_buffer: indices.clone(),
            index_buffer_offset: u64::MIN,
            instance_count: SINGLE,
            base_vertex: i32::default(),
            base_instance: u32::MIN,
        },
        RenderCommand::DrawIndexedPrimitives {
            primitive_type: PrimitiveType::Triangle,
            index_count: COVERING_VERTICES,
            index_type: IndexType::UInt32,
            index_buffer: indices,
            index_buffer_offset: u64::MAX - 3,
            instance_count: SINGLE,
            base_vertex: i32::default(),
            base_instance: u32::MIN,
        },
    ];
    for draw in draws {
        refuses_draw(&mut subject, &pass(&drawn), &state, vec![draw]);
    }
    let counting = RenderCommand::SetVisibilityResultMode {
        mode: VisibilityResultMode::Counting,
        offset: u64::MAX - 7,
    };
    refuses_draw(&mut subject, &counted, &state, vec![counting]);
}

/// Checks that alpha to coverage into an integer format is refused, where Metal stops the process.
pub fn refuses_alpha_to_coverage_into_an_integer_format<M: Metal>(mut subject: M) {
    let vertex = function(&mut subject, naga::ShaderStage::Vertex, VERTEX);
    let whole = function(&mut subject, naga::ShaderStage::Fragment, WHOLE);
    let mut descriptor = pipeline_descriptor::<M>(
        &vertex,
        &whole,
        Some((PixelFormat::Rgba8Uint, None)),
        None,
        None,
    );
    descriptor.alpha_to_coverage_enabled = true;
    assert!(matches!(
        subject.make_render_pipeline_state(&descriptor),
        Err(MakeRenderPipelineStateError::Invalid(_))
    ));
}

/// Checks that a sampler clamping to zero and to the border color at once is refused, which Metal's validation stops.
pub fn refuses_a_sampler_clamping_both_to_zero_and_to_the_border<M: Metal>(mut subject: M) {
    let (zero, border) = (
        SamplerAddressMode::ClampToZero,
        SamplerAddressMode::ClampToBorderColor,
    );
    let edge = SamplerAddressMode::ClampToEdge;
    for modes in [
        [zero, border, edge],
        [edge, zero, border],
        [border, edge, zero],
    ] {
        assert!(matches!(
            sampler_of(&mut subject, modes),
            Err(MakeSamplerStateError::Invalid(_))
        ));
    }
    assert!(sampler_of(&mut subject, [zero, zero, edge]).is_ok());
    assert!(sampler_of(&mut subject, [border, border, edge]).is_ok());
}

/// Checks that one texture as two color attachments of a pass is refused, which Metal's validation stops.
pub fn refuses_one_texture_as_two_attachments<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    let mut twice = pass(&texture);
    twice.color_attachments[1] = Some(RenderPassColorAttachment {
        texture: &texture,
        level: u32::MIN,
        slice: u32::MIN,
        depth_plane: u32::MIN,
        load_action: LoadAction::Load,
        store_action: StoreAction::Store,
        clear_color: [0.0; 4],
        resolve: None,
    });
    refuses_draw(&mut subject, &twice, &state, Vec::new());
}

/// Checks that an empty byte write at a slot a function reads is refused, which Metal's validation stops.
pub fn refuses_an_empty_byte_write_at_a_slot_a_function_reads<M: Metal>(mut subject: M) {
    refuses_dispatch(
        &mut subject,
        vec![ComputeCommand::SetBytes {
            bytes: Vec::new(),
            index: WORDS_SLOT,
        }],
    );
}

/// Checks that an integer texture at a slot a function samples as floats is refused, which Metal's validation stops.
pub fn refuses_an_integer_texture_at_a_float_slot<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, SAMPLED);
    let drawn = target(&mut subject);
    let integers = made(
        &mut subject,
        described(TextureType::Type2D, PixelFormat::Rgba8Uint),
    );
    let sampler = sampler_of(&mut subject, [SamplerAddressMode::ClampToEdge; 3])
        .expect("Metal answers make sampler state");
    let bound = vec![
        RenderCommand::SetFragmentTexture {
            texture: integers,
            index: u32::MIN,
        },
        RenderCommand::SetFragmentSamplerState {
            sampler,
            index: u32::MIN,
        },
    ];
    refuses_draw(&mut subject, &pass(&drawn), &state, bound);
}

/// Checks that a runtime-sized array bound shorter than one element is refused, and one element taken.
///
/// naga's bound of such an array subtracts one element from the bound length, which wraps on a shorter one and lets the
/// function reach past the bound range.
pub fn refuses_a_runtime_array_shorter_than_one_element<M: Metal>(mut subject: M) {
    let bytes = pattern((WORDS_BYTES * 2) as usize);
    let buffer = filled(&mut subject, &bytes);
    for length in [0, 1, 3] {
        refuses_dispatch(
            &mut subject,
            vec![ComputeCommand::SetBuffer {
                buffer: buffer.clone(),
                offset: WORDS_BYTES,
                length,
                index: WORDS_SLOT,
            }],
        );
    }
    let compute = function(&mut subject, naga::ShaderStage::Compute, DOUBLE);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_compute_commands(
            &command_buffer,
            &[
                ComputeCommand::SetComputePipelineState(state),
                ComputeCommand::SetBuffer {
                    buffer: buffer.clone(),
                    offset: WORDS_BYTES,
                    length: RGBA8_BYTES,
                    index: WORDS_SLOT,
                },
                ComputeCommand::DispatchThreadgroups(Size {
                    width: 2,
                    height: SINGLE,
                    depth: SINGLE,
                }),
            ],
        )
        .expect("Metal answers encode compute commands");
    finish(&mut subject, &command_buffer);
    // Only the one bound word may change, though eight threads write.
    let mut read = vec![u8::MIN; bytes.len()];
    subject
        .read_contents(&buffer, u64::MIN, &mut read)
        .expect("Metal answers read contents");
    let bound = WORDS_BYTES as usize..(WORDS_BYTES + RGBA8_BYTES) as usize;
    let outside = |b: &[u8]| -> Vec<u8> {
        b.iter()
            .enumerate()
            .filter(|(i, _)| !bound.contains(i))
            .map(|(_, v)| *v)
            .collect()
    };
    assert_eq!(outside(&read), outside(&bytes));
    assert_ne!(read[bound.clone()], bytes[bound]);
}

/// The least bytes of one buffer Metal allows on every device.
///
/// https://developer.apple.com/documentation/metal/mtldevice/maxbufferlength
const LEAST_MAX_BUFFER_LENGTH: u64 = 256 << 20;

/// Checks that a buffer one byte longer than the device's most is refused, and that the most is at least Metal's least.
pub fn refuses_a_buffer_past_the_max_buffer_length<M: Metal>(mut subject: M) {
    let most = subject.max_buffer_length();
    assert!(
        most >= LEAST_MAX_BUFFER_LENGTH,
        "a max buffer length of {most}"
    );
    for length in [most.saturating_add(1), u64::MAX] {
        assert!(matches!(
            subject.make_buffer(length),
            Err(MakeBufferError::TooLong)
        ));
    }
}

/// Checks that a cube of an array length other than one is refused, which Metal's validation stops.
pub fn refuses_a_cube_of_more_than_one_slice<M: Metal>(mut subject: M) {
    let mut descriptor = described(TextureType::TypeCube, PixelFormat::Rgba8Unorm);
    descriptor.array_length = 5;
    refuses_texture(&mut subject, descriptor);
    descriptor.array_length = SINGLE;
    made(&mut subject, descriptor);
}

/// Returns a color attachment of a pipeline of `format`, blending by `factor` and writing the channels of `mask`.
fn color_of(
    format: PixelFormat,
    factor: Option<BlendFactor>,
    mask: [bool; 4],
) -> RenderPipelineColorAttachmentDescriptor {
    RenderPipelineColorAttachmentDescriptor {
        pixel_format: format,
        blending: factor.map(|factor| Blending {
            rgb_blend_operation: BlendOperation::Add,
            alpha_blend_operation: BlendOperation::Add,
            source_rgb_blend_factor: factor,
            destination_rgb_blend_factor: BlendFactor::Zero,
            source_alpha_blend_factor: BlendFactor::One,
            destination_alpha_blend_factor: BlendFactor::Zero,
        }),
        write_mask: ColorWriteMask {
            red: mask[0],
            green: mask[1],
            blue: mask[2],
            alpha: mask[3],
        },
    }
}

/// Returns whether a pipeline drawing the uniform's color into `colors` is made.
fn makes_pipeline<M: Metal>(
    subject: &mut M,
    colors: [Option<RenderPipelineColorAttachmentDescriptor>; MAX_COLOR_ATTACHMENTS],
) -> bool {
    let vertex = function(subject, naga::ShaderStage::Vertex, VERTEX);
    let fragment = function(subject, naga::ShaderStage::Fragment, FLAT);
    let mut descriptor = pipeline_descriptor::<M>(&vertex, &fragment, None, None, None);
    descriptor.color_attachments = colors;
    subject.make_render_pipeline_state(&descriptor).is_ok()
}

/// Checks that blending by the second source in a pipeline of more than its first color attachment is refused.
///
/// Metal's validation stops such a pipeline.
pub fn refuses_a_second_source_blend_beside_another_attachment<M: Metal>(mut subject: M) {
    let all = [true; 4];
    let second = Some(BlendFactor::Source1Alpha);
    let mut colors = [None; MAX_COLOR_ATTACHMENTS];
    colors[0] = Some(color_of(PixelFormat::Rgba8Unorm, second, all));
    colors[1] = Some(color_of(PixelFormat::Rgba8Unorm, None, all));
    assert!(!makes_pipeline(&mut subject, colors));
    colors[0] = Some(color_of(PixelFormat::Rgba8Unorm, None, all));
    colors[3] = Some(color_of(PixelFormat::Rgba8Unorm, second, all));
    assert!(!makes_pipeline(&mut subject, colors));
}

/// Checks that writing some channels of a shared-exponent target is refused, and all of them is not.
///
/// Metal's validation stops a pipeline of such a mask.
pub fn refuses_some_channels_of_a_shared_exponent_target<M: Metal>(mut subject: M) {
    let mut colors = [None; MAX_COLOR_ATTACHMENTS];
    colors[0] = Some(color_of(
        PixelFormat::Rgb9e5Float,
        None,
        [false, true, true, true],
    ));
    assert!(!makes_pipeline(&mut subject, colors));
    colors[0] = Some(color_of(PixelFormat::Rgb9e5Float, None, [true; 4]));
    assert!(makes_pipeline(&mut subject, colors));
}

/// Checks that bytes at a slot the function writes are refused, which Metal's validation stops.
pub fn refuses_bytes_at_a_slot_the_function_writes<M: Metal>(mut subject: M) {
    refuses_dispatch(
        &mut subject,
        vec![ComputeCommand::SetBytes {
            bytes: pattern(WORDS_BYTES as usize),
            index: WORDS_SLOT,
        }],
    );
}

/// Checks that a vertex buffer of a stride past `MAX_VERTEX_STRIDE` is refused, and one of that stride is not.
pub fn refuses_a_vertex_stride_past_the_most<M: Metal>(mut subject: M) {
    let (module, info) = programs();
    for (stride, made) in [
        (MAX_VERTEX_STRIDE.saturating_add(4), false),
        (u32::MAX, false),
        (MAX_VERTEX_STRIDE, true),
    ] {
        let vertex_buffers = [VertexBufferLayout {
            buffer_index: VERTICES,
            stride,
            step_function: VertexStepFunction::PerVertex,
            step_rate: 1,
            attributes: vec![VertexAttribute {
                location: u32::MIN,
                offset: u32::MIN,
                format: VertexFormat::Float2,
            }],
        }];
        let function = subject.make_function(&FunctionSource {
            module: &module,
            info: &info,
            stage: naga::ShaderStage::Vertex,
            entry_point: VERTEX,
            resources: SLOTS,
            immediates_buffer_index: IMMEDIATES,
            sizes_buffer_index: SIZES,
            vertex_buffers: &vertex_buffers,
            point_size: false,
        });
        assert_eq!(function.is_ok(), made, "a stride of {stride}");
    }
}

/// Checks that a visibility offset Metal's validation stops is refused with counting off and no buffer too.
pub fn refuses_a_visibility_offset_with_no_buffer<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    for offset in [15, 1 << 63, BEYOND_VISIBILITY_SPAN] {
        let off = RenderCommand::SetVisibilityResultMode {
            mode: VisibilityResultMode::Disabled,
            offset,
        };
        refuses_draw(&mut subject, &pass(&texture), &state, vec![off]);
    }
}

/// Checks that a copy into a compressed level past its texels, into the rest of its last block, is refused.
///
/// Metal's validation stops such a copy; one that ends at the level's texels is taken.
pub fn refuses_a_copy_past_a_compressed_levels_texels<M: Metal>(mut subject: M) {
    let mut descriptor = described(TextureType::Type2D, PixelFormat::Bc1Rgba);
    // Its second level is four texels wide and one high, inside one block.
    descriptor.width = TARGET_SIDE * 2;
    descriptor.height = 2;
    descriptor.mipmap_level_count = 2;
    descriptor.usage.render_target = false;
    let texture = made(&mut subject, descriptor);
    let buffer = filled(&mut subject, &pattern(256));
    for (height, taken) in [(TARGET_SIDE, false), (SINGLE, true)] {
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        let copied = subject.encode_blit_commands(
            &command_buffer,
            &[BlitCommand::CopyFromBufferToTexture {
                source_buffer: buffer.clone(),
                source_offset: u64::MIN,
                source_bytes_per_row: 8,
                source_bytes_per_image: 8,
                source_size: Size {
                    width: TARGET_SIDE,
                    height,
                    depth: SINGLE,
                },
                destination_texture: texture.clone(),
                destination_slice: u32::MIN,
                destination_level: 1,
                destination_origin: Origin {
                    x: u32::MIN,
                    y: u32::MIN,
                    z: u32::MIN,
                },
                options: BlitOption::None,
            }],
        );
        assert_eq!(copied.is_ok(), taken, "{height} rows: {copied:?}");
        if taken {
            finish(&mut subject, &command_buffer);
        }
    }
}

/// Checks that a pass of no attachment wider or higher than a target's longest side is refused.
///
/// The GPU ends such a pass's work in an error.
pub fn refuses_a_pass_of_no_attachment_past_the_longest_side<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    let mut empty = pass(&texture);
    empty.color_attachments = [const { None }; MAX_COLOR_ATTACHMENTS];
    for (width, height) in [
        (u32::MAX - 3, TARGET_SIDE),
        (TARGET_SIDE, MAX_TEXTURE_SIDE + 1),
    ] {
        empty.render_target_width = width;
        empty.render_target_height = height;
        refuses_draw(&mut subject, &empty, &state, Vec::new());
    }
}

/// Checks that a draw whose vertex buffer holds less than one element, as a buffer or as bytes, completes.
///
/// Metal's validation stops such a draw unless what Metal binds holds one element; the function reads none of it.
pub fn draws_from_a_vertex_buffer_shorter_than_one_element<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    let depth_stencil = no_depth(&mut subject);
    let short = filled(&mut subject, &pattern(4));
    let vertices = covering(&mut subject);
    let color = filled(&mut subject, &[u8::MIN; COLOR_BYTES as usize]);
    for bytes in [false, true] {
        let shorter = if bytes {
            RenderCommand::SetVertexBytes {
                bytes: pattern(4),
                index: VERTICES,
            }
        } else {
            RenderCommand::SetVertexBuffer {
                buffer: short.clone(),
                offset: u64::MIN,
                length: 4,
                index: VERTICES,
            }
        };
        let bindings = vec![
            RenderCommand::SetFragmentBuffer {
                buffer: color.clone(),
                offset: u64::MIN,
                length: COLOR_BYTES,
                index: u32::MIN,
            },
            shorter,
        ];
        // The covering triangle's binding comes first, and the shorter one replaces it.
        let commands = covering_draw(&state, &vertices, &depth_stencil, bindings);
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        subject
            .encode_render_commands(&command_buffer, &pass(&texture), &commands)
            .expect("Metal answers encode render commands");
        finish(&mut subject, &command_buffer);
    }
}

/// Checks that a grid whose threads on one axis pass the largest `u32` is refused, and one of exactly that many is not.
///
/// Metal's validation stops such a grid, whose thread positions would not fit the `u32` a function reads them as.
pub fn refuses_a_grid_of_more_threads_on_one_axis_than_a_u32_holds<M: Metal>(mut subject: M) {
    let buffer = filled(&mut subject, &pattern(WORDS_BYTES as usize));
    let compute = function(&mut subject, naga::ShaderStage::Compute, DOUBLE);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    // The function's threadgroup is four threads wide and one high and deep.
    let widest = u32::MAX / 4;
    for (groups, refused) in [
        ((widest.saturating_add(1), SINGLE, SINGLE), true),
        ((u32::MAX, SINGLE, SINGLE), true),
        ((widest, SINGLE, SINGLE), false),
        ((SINGLE, u32::MAX, SINGLE), false),
        ((SINGLE, SINGLE, u32::MAX), false),
    ] {
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        // A grid that is not refused is encoded and never committed, so the GPU runs none of it.
        let encoded = subject.encode_compute_commands(
            &command_buffer,
            &[
                ComputeCommand::SetComputePipelineState(state.clone()),
                ComputeCommand::SetBuffer {
                    buffer: buffer.clone(),
                    offset: u64::MIN,
                    length: WORDS_BYTES,
                    index: WORDS_SLOT,
                },
                ComputeCommand::DispatchThreadgroups(Size {
                    width: groups.0,
                    height: groups.1,
                    depth: groups.2,
                }),
            ],
        );
        assert_eq!(
            encoded.is_err(),
            refused,
            "a grid of {groups:?} groups: {encoded:?}"
        );
    }
}

/// Returns the little-endian bytes of words.
fn word_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Returns the texels of a target the covering triangle drew green into, and of one it drew nothing into.
fn green_and_clear() -> (Vec<u8>, Vec<u8>) {
    let texels = (TARGET_SIDE * TARGET_SIDE) as usize;
    (
        rgba8_unorm(GREEN).repeat(texels),
        vec![u8::MIN; texels * RGBA8_BYTES as usize],
    )
}

/// The covering triangle's three vertices as 32-bit indices.
///
/// https://developer.apple.com/documentation/metal/mtlprimitivetype/triangle
const COVERING_INDICES: [u32; 3] = [0, 1, 2];

/// The covering triangle's vertices plus one as 32-bit indices, then its vertices.
///
/// https://developer.apple.com/documentation/metal/mtlprimitivetype/triangle
const SHIFTED_INDICES: [u32; 6] = [1, 2, 3, 0, 1, 2];

/// Returns the commands of the covering draw in green, its draw replaced by `draw`.
fn green_draw<M: Metal>(
    subject: &mut M,
    state: &M::RenderPipelineState,
    draw: RenderCommand<M>,
) -> Vec<RenderCommand<M>> {
    let depth_stencil = no_depth(subject);
    let vertices = covering(subject);
    let color = filled(subject, &color_bytes(GREEN));
    let mut commands = covering_draw(
        state,
        &vertices,
        &depth_stencil,
        vec![RenderCommand::SetFragmentBuffer {
            buffer: color,
            offset: u64::MIN,
            length: COLOR_BYTES,
            index: u32::MIN,
        }],
    );
    commands.pop();
    commands.push(draw);
    commands
}

/// Draws the covering triangle in green with `draw` into a new target and returns its texels.
fn drawn_with<M: Metal>(
    subject: &mut M,
    state: &M::RenderPipelineState,
    draw: RenderCommand<M>,
) -> Vec<u8> {
    let commands = green_draw(subject, state, draw);
    let texture = target(subject);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_render_commands(&command_buffer, &pass(&texture), &commands)
        .expect("Metal answers encode render commands");
    finish(subject, &command_buffer);
    read_texture(subject, &texture)
}

/// Returns the bytes the GPU reads of an indirect draw's arguments, each little-endian in the order of Metal's.
fn draw_bytes(arguments: DrawPrimitivesIndirectArguments) -> Vec<u8> {
    let DrawPrimitivesIndirectArguments {
        vertex_count,
        instance_count,
        vertex_start,
        base_instance,
    } = arguments;
    word_bytes(&[vertex_count, instance_count, vertex_start, base_instance])
}

/// Returns the bytes the GPU reads of an indirect indexed draw's arguments, in the order of Metal's structure.
fn indexed_draw_bytes(arguments: DrawIndexedPrimitivesIndirectArguments) -> Vec<u8> {
    let DrawIndexedPrimitivesIndirectArguments {
        index_count,
        instance_count,
        index_start,
        base_vertex,
        base_instance,
    } = arguments;
    [
        index_count.to_le_bytes(),
        instance_count.to_le_bytes(),
        index_start.to_le_bytes(),
        base_vertex.to_le_bytes(),
        base_instance.to_le_bytes(),
    ]
    .concat()
}

/// Returns the bytes the GPU reads of an indirect dispatch's arguments, in the order of Metal's structure.
fn dispatch_bytes(arguments: DispatchThreadgroupsIndirectArguments) -> Vec<u8> {
    let Size {
        width,
        height,
        depth,
    } = arguments.threadgroups_per_grid;
    word_bytes(&[width, height, depth])
}

// Each structure is as long as the bytes its indirect call reads.
const _: () = assert!(size_of::<DrawPrimitivesIndirectArguments>() as u64 == DRAW_ARGUMENTS_BYTES);
const _: () = assert!(
    size_of::<DrawIndexedPrimitivesIndirectArguments>() as u64 == DRAW_INDEXED_ARGUMENTS_BYTES
);
const _: () =
    assert!(size_of::<DispatchThreadgroupsIndirectArguments>() as u64 == DISPATCH_ARGUMENTS_BYTES);

/// One draw of the covering triangle, as the arguments of an indirect draw.
///
/// https://developer.apple.com/documentation/metal/mtldrawprimitivesindirectarguments
const COVERING_DRAW: DrawPrimitivesIndirectArguments = DrawPrimitivesIndirectArguments {
    vertex_count: COVERING_VERTICES,
    instance_count: 1,
    vertex_start: 0,
    base_instance: 0,
};

/// The indirect draws of the counts test, each with whether it draws the covering triangle.
///
/// Only a draw of some vertices and some instances, whose last vertex and last instance a `u32` holds, draws.
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/drawprimitives(type:indirectbuffer:indirectbufferoffset:)
const INDIRECT_DRAWS: [(DrawPrimitivesIndirectArguments, bool); 8] = [
    (COVERING_DRAW, true),
    (
        DrawPrimitivesIndirectArguments {
            base_instance: u32::MAX - 1,
            ..COVERING_DRAW
        },
        true,
    ),
    (
        DrawPrimitivesIndirectArguments {
            instance_count: 2,
            base_instance: u32::MAX - 1,
            ..COVERING_DRAW
        },
        false,
    ),
    (
        DrawPrimitivesIndirectArguments {
            vertex_start: u32::MAX - 1,
            ..COVERING_DRAW
        },
        false,
    ),
    (
        DrawPrimitivesIndirectArguments {
            instance_count: 2,
            base_instance: u32::MAX,
            ..COVERING_DRAW
        },
        false,
    ),
    (
        DrawPrimitivesIndirectArguments {
            vertex_count: u32::MAX,
            instance_count: u32::MAX,
            vertex_start: u32::MAX,
            base_instance: u32::MAX,
        },
        false,
    ),
    (
        DrawPrimitivesIndirectArguments {
            vertex_count: 0,
            ..COVERING_DRAW
        },
        false,
    ),
    (
        DrawPrimitivesIndirectArguments {
            instance_count: 0,
            ..COVERING_DRAW
        },
        false,
    ),
];

/// One indexed draw of the covering triangle, as the arguments of an indirect indexed draw.
///
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const COVERING_INDEXED_DRAW: DrawIndexedPrimitivesIndirectArguments =
    DrawIndexedPrimitivesIndirectArguments {
        index_count: COVERING_VERTICES,
        instance_count: 1,
        index_start: 0,
        base_vertex: 0,
        base_instance: 0,
    };

/// The byte offset of the covering triangle's own indices in `SHIFTED_INDICES`.
///
/// https://developer.apple.com/documentation/metal/mtlindextype/uint32
const SHIFTED_OWN_OFFSET: u64 = COVERING_VERTICES as u64 * size_of::<u32>() as u64;

/// The indexed draws of the counts test from `SHIFTED_INDICES`, each with its index offset and whether it draws.
///
/// The shifted indices with a base vertex of minus one draw the triangle; a draw reaching past the indices bound, or
/// whose first index or first instance plus its count passes the largest `u32`, draws nothing.
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const INDIRECT_INDEXED_DRAWS: [(DrawIndexedPrimitivesIndirectArguments, u64, bool); 10] = [
    (COVERING_INDEXED_DRAW, SHIFTED_OWN_OFFSET, true),
    (
        DrawIndexedPrimitivesIndirectArguments {
            base_vertex: -1,
            ..COVERING_INDEXED_DRAW
        },
        0,
        true,
    ),
    (
        DrawIndexedPrimitivesIndirectArguments {
            base_instance: u32::MAX - 1,
            ..COVERING_INDEXED_DRAW
        },
        SHIFTED_OWN_OFFSET,
        true,
    ),
    (
        DrawIndexedPrimitivesIndirectArguments {
            index_start: 1,
            ..COVERING_INDEXED_DRAW
        },
        SHIFTED_OWN_OFFSET,
        false,
    ),
    (
        COVERING_INDEXED_DRAW,
        SHIFTED_OWN_OFFSET + INDIRECT_ARGUMENTS_ALIGNMENT,
        false,
    ),
    (ONE_INDEX_DRAW, 2 * SHIFTED_OWN_OFFSET, false),
    (
        DrawIndexedPrimitivesIndirectArguments {
            index_count: 0,
            ..COVERING_INDEXED_DRAW
        },
        2 * SHIFTED_OWN_OFFSET,
        false,
    ),
    (
        DrawIndexedPrimitivesIndirectArguments {
            index_count: 2,
            index_start: u32::MAX,
            ..COVERING_INDEXED_DRAW
        },
        SHIFTED_OWN_OFFSET,
        false,
    ),
    (
        DrawIndexedPrimitivesIndirectArguments {
            instance_count: 2,
            base_instance: u32::MAX - 1,
            ..COVERING_INDEXED_DRAW
        },
        SHIFTED_OWN_OFFSET,
        false,
    ),
    (
        DrawIndexedPrimitivesIndirectArguments {
            index_count: u32::MAX,
            ..COVERING_INDEXED_DRAW
        },
        SHIFTED_OWN_OFFSET,
        false,
    ),
];

/// An indexed draw of one index.
///
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const ONE_INDEX_DRAW: DrawIndexedPrimitivesIndirectArguments =
    DrawIndexedPrimitivesIndirectArguments {
        index_count: 1,
        ..COVERING_INDEXED_DRAW
    };

/// Checks that draws read their counts from a buffer when they run.
///
/// One reaching past what is bound, or whose first index or instance plus its count passes the largest `u32`, draws
/// nothing.
pub fn draws_with_counts_the_gpu_reads<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let (green, clear) = green_and_clear();
    for (arguments, draws) in INDIRECT_DRAWS {
        let buffer = filled(&mut subject, &draw_bytes(arguments));
        let draw = RenderCommand::DrawPrimitivesIndirect {
            primitive_type: PrimitiveType::Triangle,
            indirect_buffer: buffer,
            indirect_buffer_offset: u64::MIN,
        };
        assert_eq!(
            &drawn_with(&mut subject, &state, draw),
            if draws { &green } else { &clear },
            "a draw of arguments {arguments:?}"
        );
    }
    let indices = filled(&mut subject, &word_bytes(&SHIFTED_INDICES));
    for (arguments, index_offset, draws) in INDIRECT_INDEXED_DRAWS {
        let buffer = filled(&mut subject, &indexed_draw_bytes(arguments));
        let draw = RenderCommand::DrawIndexedPrimitivesIndirect {
            primitive_type: PrimitiveType::Triangle,
            index_type: IndexType::UInt32,
            index_buffer: indices.clone(),
            index_buffer_offset: index_offset,
            indirect_buffer: buffer,
            indirect_buffer_offset: u64::MIN,
        };
        assert_eq!(
            &drawn_with(&mut subject, &state, draw),
            if draws { &green } else { &clear },
            "an indexed draw of arguments {arguments:?} from index offset {index_offset}"
        );
    }
}

/// Checks that a draw reads the counts a copy before it wrote, and that a write to them from the CPU waits for it.
///
/// The write is refused while the draw is not committed, and lands after it once it is.
pub fn draws_with_counts_written_before_them_and_waits_for_writes<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let (green, _) = green_and_clear();
    let written = filled(&mut subject, &draw_bytes(COVERING_DRAW));
    let arguments = filled(&mut subject, &[u8::MIN; 16]);
    let draw = RenderCommand::DrawPrimitivesIndirect {
        primitive_type: PrimitiveType::Triangle,
        indirect_buffer: arguments.clone(),
        indirect_buffer_offset: u64::MIN,
    };
    let commands = green_draw(&mut subject, &state, draw);
    let texture = target(&mut subject);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::CopyFromBufferToBuffer {
                source_buffer: written,
                source_offset: u64::MIN,
                destination_buffer: arguments.clone(),
                destination_offset: u64::MIN,
                size: 16,
            }],
        )
        .expect("Metal answers encode blit commands");
    subject
        .encode_render_commands(&command_buffer, &pass(&texture), &commands)
        .expect("Metal answers encode render commands");
    assert_eq!(
        subject.write_contents(&arguments, u64::MIN, &[u8::MIN; 16]),
        Err(WriteContentsError::UsedByUncommittedWork)
    );
    subject
        .commit(&command_buffer)
        .expect("Metal answers commit");
    subject
        .write_contents(&arguments, u64::MIN, &[u8::MIN; 16])
        .expect("Metal answers write contents");
    assert_eq!(
        subject.status(&command_buffer),
        CommandBufferStatus::Completed
    );
    assert_eq!(read_texture(&mut subject, &texture), green);
}

/// The threads of a threadgroup of `DOUBLE` on its first axis, as its `workgroup_size` says.
///
/// https://www.w3.org/TR/WGSL/#attribute-workgroup_size
const DOUBLE_GROUP_WIDTH: u32 = 4;

/// Returns the arguments of an indirect dispatch of `width` by `height` by `depth` threadgroups.
const fn groups(width: u32, height: u32, depth: u32) -> DispatchThreadgroupsIndirectArguments {
    DispatchThreadgroupsIndirectArguments {
        threadgroups_per_grid: Size {
            width,
            height,
            depth,
        },
    }
}

/// The dispatches of `DOUBLE` the GPU reads, each with the most threadgroups it may run and whether it doubles.
///
/// Only the two groups that cover the words, within the most, double them; more groups on an axis than the most, more
/// threads on an axis than a `u32` holds, or no groups, dispatch nothing.
/// https://developer.apple.com/documentation/metal/mtldispatchthreadgroupsindirectarguments
const INDIRECT_DISPATCHES: [(DispatchThreadgroupsIndirectArguments, u32, bool); 7] = [
    (groups(2, 1, 1), 2, true),
    (groups(3, 1, 1), 2, false),
    (groups(2, 1, 3), 2, false),
    (groups(1, 1, 1), 0, false),
    (
        groups(u32::MAX / DOUBLE_GROUP_WIDTH + 1, 1, 1),
        u32::MAX,
        false,
    ),
    (groups(u32::MAX, 1, 1), u32::MAX, false),
    (groups(2, 1, 0), 2, false),
];

/// Checks that a dispatch reads its threadgroups from a buffer when it runs.
///
/// One of more groups on an axis than its most, or of more threads on an axis than a `u32` holds, dispatches nothing.
pub fn dispatches_threadgroups_the_gpu_reads<M: Metal>(mut subject: M) {
    let words: Vec<u32> = (1..=WORD_COUNT as u32).collect();
    let compute = function(&mut subject, naga::ShaderStage::Compute, DOUBLE);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    for (groups, most, doubled) in INDIRECT_DISPATCHES {
        let buffer = filled(&mut subject, &word_bytes(&words));
        let arguments = filled(&mut subject, &dispatch_bytes(groups));
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        subject
            .encode_compute_commands(
                &command_buffer,
                &[
                    ComputeCommand::SetComputePipelineState(state.clone()),
                    ComputeCommand::SetBuffer {
                        buffer: buffer.clone(),
                        offset: u64::MIN,
                        length: WORDS_BYTES,
                        index: WORDS_SLOT,
                    },
                    ComputeCommand::DispatchThreadgroupsIndirect {
                        indirect_buffer: arguments,
                        indirect_buffer_offset: u64::MIN,
                        max_threadgroups: most,
                    },
                ],
            )
            .expect("Metal answers encode compute commands");
        finish(&mut subject, &command_buffer);
        let mut read = vec![u8::MIN; WORDS_BYTES as usize];
        subject
            .read_contents(&buffer, u64::MIN, &mut read)
            .expect("Metal answers read contents");
        let expected: Vec<u32> = if doubled {
            words.iter().map(|w| w * 2).collect()
        } else {
            words.clone()
        };
        assert_eq!(
            read,
            word_bytes(&expected),
            "a dispatch of {groups:?} groups, at most {most}"
        );
    }
}

/// The bytes of the buffer of indirect arguments the refusals read: two draws' arguments.
///
/// https://developer.apple.com/documentation/metal/mtldrawprimitivesindirectarguments
const INDIRECT_SPAN: u64 = 2 * DRAW_ARGUMENTS_BYTES;

/// Offsets of a draw's arguments in `INDIRECT_SPAN` bytes that are refused: off four bytes, or reaching past the end.
///
/// https://developer.apple.com/documentation/metal/mtlrendercommandencoder/drawprimitives(type:indirectbuffer:indirectbufferoffset:)
const REFUSED_DRAW_OFFSETS: [u64; 5] = [
    2,
    DRAW_ARGUMENTS_BYTES + 1,
    INDIRECT_SPAN - DRAW_ARGUMENTS_BYTES + INDIRECT_ARGUMENTS_ALIGNMENT,
    INDIRECT_SPAN,
    INDIRECT_SPAN + INDIRECT_ARGUMENTS_ALIGNMENT,
];

/// Offsets of an indexed draw's arguments in `INDIRECT_SPAN` bytes, and of its three indices, that are refused.
///
/// Each is off its size, reaches past its buffer's end, or wraps.
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const REFUSED_INDEXED_DRAW_OFFSETS: [(u64, u64); 8] = [
    (2, 0),
    (INDIRECT_SPAN - DRAW_INDEXED_ARGUMENTS_BYTES + 1, 0),
    (
        INDIRECT_SPAN - DRAW_INDEXED_ARGUMENTS_BYTES + INDIRECT_ARGUMENTS_ALIGNMENT,
        0,
    ),
    (u64::MAX - 3, 0),
    (0, 2),
    (0, SHIFTED_OWN_OFFSET + INDIRECT_ARGUMENTS_ALIGNMENT),
    (0, u64::MAX - 3),
    (0, u64::MAX),
];

/// Offsets of a dispatch's arguments in `INDIRECT_SPAN` bytes that are refused.
///
/// Each is off four bytes, reaches past the end, or wraps.
/// https://developer.apple.com/documentation/metal/mtldispatchthreadgroupsindirectarguments
const REFUSED_DISPATCH_OFFSETS: [u64; 5] = [
    2,
    INDIRECT_SPAN - DISPATCH_ARGUMENTS_BYTES + 1,
    INDIRECT_SPAN - DISPATCH_ARGUMENTS_BYTES + INDIRECT_ARGUMENTS_ALIGNMENT,
    u64::MAX - 3,
    u64::MAX,
];

/// Checks that indirect arguments outside their buffer are refused before anything is encoded.
///
/// So are an indexed draw's index offset outside its buffer, and either off its alignment or with an end that wraps.
pub fn refuses_indirect_arguments_outside_their_buffer<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let texture = target(&mut subject);
    // One draw's arguments, then zeros to `INDIRECT_SPAN`.
    let mut words = draw_bytes(COVERING_DRAW);
    words.resize(INDIRECT_SPAN as usize, u8::MIN);
    let arguments = filled(&mut subject, &words);
    let indices = filled(&mut subject, &word_bytes(&COVERING_INDICES));
    for offset in REFUSED_DRAW_OFFSETS.into_iter().chain(WRAPPING_OFFSETS) {
        let draw = RenderCommand::DrawPrimitivesIndirect {
            primitive_type: PrimitiveType::Triangle,
            indirect_buffer: arguments.clone(),
            indirect_buffer_offset: offset,
        };
        refuses_draw(&mut subject, &pass(&texture), &state, vec![draw]);
    }
    for (offset, index_offset) in REFUSED_INDEXED_DRAW_OFFSETS {
        let draw = RenderCommand::DrawIndexedPrimitivesIndirect {
            primitive_type: PrimitiveType::Triangle,
            index_type: IndexType::UInt32,
            index_buffer: indices.clone(),
            index_buffer_offset: index_offset,
            indirect_buffer: arguments.clone(),
            indirect_buffer_offset: offset,
        };
        refuses_draw(&mut subject, &pass(&texture), &state, vec![draw]);
    }
    let buffer = filled(&mut subject, &pattern(WORDS_BYTES as usize));
    let compute = function(&mut subject, naga::ShaderStage::Compute, DOUBLE);
    let compute_state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    for offset in REFUSED_DISPATCH_OFFSETS {
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        assert!(matches!(
            subject.encode_compute_commands(
                &command_buffer,
                &[
                    ComputeCommand::SetComputePipelineState(compute_state.clone()),
                    ComputeCommand::SetBuffer {
                        buffer: buffer.clone(),
                        offset: u64::MIN,
                        length: WORDS_BYTES,
                        index: WORDS_SLOT,
                    },
                    ComputeCommand::DispatchThreadgroupsIndirect {
                        indirect_buffer: arguments.clone(),
                        indirect_buffer_offset: offset,
                        max_threadgroups: u32::MAX,
                    },
                ],
            ),
            Err(EncodeComputeCommandsError::OutOfRange(_))
        ));
    }
    // With no pipeline bound the dispatch is refused too.
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    assert!(matches!(
        subject.encode_compute_commands(
            &command_buffer,
            &[ComputeCommand::DispatchThreadgroupsIndirect {
                indirect_buffer: arguments,
                indirect_buffer_offset: u64::MIN,
                max_threadgroups: u32::MAX,
            }],
        ),
        Err(EncodeComputeCommandsError::Unbound(_))
    ));
}

/// The step rate test's functions: a column of the target for each instance.
///
/// Each column is at the place its first buffer gives, in the red its second gives.
const COLUMNS: &str = "
struct Out { @builtin(position) pos: vec4<f32>, @location(0) @interpolate(flat) red: f32 }
@vertex fn columns(@builtin(vertex_index) v: u32, @location(0) column: f32, @location(1) red: f32) -> Out {
    var corners = array(vec2(0.0, -1.0), vec2(1.0, -1.0), vec2(0.0, 1.0), vec2(1.0, 1.0));
    let corner = corners[v];
    return Out(vec4<f32>((column + corner.x) * 0.5 - 1.0, corner.y, 0.5, 1.0), red);
}
@fragment fn shade(i: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(i.red, 0.0, 0.0, 1.0);
}
";

/// The base instance of the step rate test's draws, past zero so that a rate that left it out would show.
///
/// https://developer.apple.com/documentation/metal/mtlvertexbufferlayoutdescriptor/steprate
const BASE_INSTANCE: u32 = 2;

/// The elements of the step rate test's buffers: as many as the base instance and one for each column.
const STEPPED_ELEMENTS: u32 = BASE_INSTANCE + TARGET_SIDE;

/// Returns the red of element `k` of the step rate test's second buffer, in bytes: ten for each element.
fn stepped_red(k: u32) -> u8 {
    u8::try_from((k + 1) * 10).expect("a red that fits")
}

/// Checks that an instance buffer advances once every `step_rate` instances from the base instance.
///
/// A rate of zero reads the base instance's element for every instance.
pub fn steps_instance_buffers_by_their_rate<M: Metal>(mut subject: M) {
    let module = naga::front::wgsl::parse_str(COLUMNS).expect("the test functions parse");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the test functions validate");
    let float_bytes = size_of::<f32>() as u32;
    // The first buffer gives instance `BASE_INSTANCE + x` column `x`; the second gives each element its own red.
    let columns: Vec<u8> = (0..STEPPED_ELEMENTS)
        .flat_map(|k| (k.saturating_sub(BASE_INSTANCE) as f32).to_le_bytes())
        .collect();
    let reds: Vec<u8> = (0..STEPPED_ELEMENTS)
        .flat_map(|k| (f32::from(stepped_red(k)) / f32::from(u8::MAX)).to_le_bytes())
        .collect();
    let columns = filled(&mut subject, &columns);
    let reds = filled(&mut subject, &reds);
    let depth_stencil = no_depth(&mut subject);
    for rate in [1, 2, 3, 0] {
        let layout = |buffer_index, location, step_rate| VertexBufferLayout {
            buffer_index,
            stride: float_bytes,
            step_function: VertexStepFunction::PerInstance,
            step_rate,
            attributes: vec![VertexAttribute {
                location,
                offset: u32::MIN,
                format: VertexFormat::Float,
            }],
        };
        let vertex_buffers = [layout(VERTICES, u32::MIN, 1), layout(VERTICES + 1, 1, rate)];
        let source = |stage, entry_point| FunctionSource {
            module: &module,
            info: &info,
            stage,
            entry_point,
            resources: &[],
            immediates_buffer_index: IMMEDIATES,
            sizes_buffer_index: SIZES,
            vertex_buffers: &vertex_buffers,
            point_size: false,
        };
        let vertex = subject
            .make_function(&source(naga::ShaderStage::Vertex, "columns"))
            .expect("Metal answers make function");
        let fragment = subject
            .make_function(&source(naga::ShaderStage::Fragment, "shade"))
            .expect("Metal answers make function");
        let state = subject
            .make_render_pipeline_state(&pipeline_descriptor::<M>(
                &vertex,
                &fragment,
                Some((PixelFormat::Rgba8Unorm, None)),
                None,
                None,
            ))
            .expect("Metal answers make render pipeline state");
        let whole = |buffer: &M::Buffer, index| RenderCommand::SetVertexBuffer {
            buffer: buffer.clone(),
            offset: u64::MIN,
            length: u64::from(STEPPED_ELEMENTS * float_bytes),
            index,
        };
        let commands = vec![
            RenderCommand::SetRenderPipelineState(state),
            RenderCommand::SetDepthStencilState(depth_stencil.clone()),
            whole(&columns, VERTICES),
            whole(&reds, VERTICES + 1),
            RenderCommand::SetViewports(vec![Viewport {
                origin_x: 0.0,
                origin_y: 0.0,
                width: f64::from(TARGET_SIDE),
                height: f64::from(TARGET_SIDE),
                znear: 0.0,
                zfar: 1.0,
            }]),
            RenderCommand::DrawPrimitives {
                primitive_type: PrimitiveType::TriangleStrip,
                vertex_start: u32::MIN,
                vertex_count: 4,
                instance_count: TARGET_SIDE,
                base_instance: BASE_INSTANCE,
            },
        ];
        let texture = target(&mut subject);
        let command_buffer = subject
            .make_command_buffer()
            .expect("Metal answers make command buffer");
        subject
            .encode_render_commands(&command_buffer, &pass(&texture), &commands)
            .expect("Metal answers encode render commands");
        finish(&mut subject, &command_buffer);
        let element = |x: u32| BASE_INSTANCE + x.checked_div(rate).unwrap_or(u32::MIN);
        let row: Vec<u8> = (0..TARGET_SIDE)
            .flat_map(|x| [stepped_red(element(x)), u8::MIN, u8::MIN, u8::MAX])
            .collect();
        assert_eq!(
            read_texture(&mut subject, &texture),
            row.repeat(TARGET_SIDE as usize),
            "a step rate of {rate}"
        );
    }
}

/// Encodes `draws` after the covering draw's bindings into one pass of a new target, and returns its texels.
fn drawn_with_all<M: Metal>(
    subject: &mut M,
    state: &M::RenderPipelineState,
    draws: Vec<RenderCommand<M>>,
) -> Vec<u8> {
    let mut draws = draws.into_iter();
    let first = draws.next().expect("one draw at least");
    let mut commands = green_draw(subject, state, first);
    commands.extend(draws);
    let texture = target(subject);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_render_commands(&command_buffer, &pass(&texture), &commands)
        .expect("Metal answers encode render commands");
    finish(subject, &command_buffer);
    read_texture(subject, &texture)
}

/// The covering triangle's three 16-bit indices and one byte more, so that the buffer's length is odd.
///
/// https://developer.apple.com/documentation/metal/mtlindextype/uint16
const ODD_LENGTH_INDICES: [u8; 7] = [0, 0, 1, 0, 2, 0, 0xff];

/// The bytes of one 16-bit index.
///
/// https://developer.apple.com/documentation/metal/mtlindextype/uint16
const INDEX_16_BYTES: u64 = size_of::<u16>() as u64;

/// Indexed draws of `ODD_LENGTH_INDICES` as 16-bit indices, each with its index buffer offset and whether it draws.
///
/// From byte 2 five bytes are left, two whole indices; from byte 6 one byte, no whole index.
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const ODD_LENGTH_DRAWS: [(DrawIndexedPrimitivesIndirectArguments, u64, bool); 6] = [
    (COVERING_INDEXED_DRAW, 0, true),
    (COVERING_INDEXED_DRAW, INDEX_16_BYTES, false),
    (
        DrawIndexedPrimitivesIndirectArguments {
            index_count: 2,
            ..COVERING_INDEXED_DRAW
        },
        INDEX_16_BYTES,
        false,
    ),
    (ONE_INDEX_DRAW, 3 * INDEX_16_BYTES, false),
    (
        DrawIndexedPrimitivesIndirectArguments {
            index_start: 1,
            ..COVERING_INDEXED_DRAW
        },
        0,
        false,
    ),
    (
        DrawIndexedPrimitivesIndirectArguments {
            index_count: u32::MAX,
            index_start: 1,
            ..COVERING_INDEXED_DRAW
        },
        0,
        false,
    ),
];

/// Index buffer offsets of `ODD_LENGTH_INDICES` that are refused: off two bytes, at the lone byte, and past the end.
///
/// https://developer.apple.com/documentation/metal/mtlindextype/uint16
const ODD_LENGTH_REFUSED_OFFSETS: [u64; 3] = [1, 7, 8];

/// Checks that an indexed draw of 16-bit indices in a buffer of an odd length counts only whole indices.
///
/// One reaching the last, lone byte draws nothing, and an offset off two bytes or past the end is refused.
pub fn draws_only_whole_16_bit_indices_of_an_odd_length_buffer<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let (green, clear) = green_and_clear();
    let indices = filled(&mut subject, &ODD_LENGTH_INDICES);
    for (arguments, index_offset, draws) in ODD_LENGTH_DRAWS {
        let buffer = filled(&mut subject, &indexed_draw_bytes(arguments));
        let draw = RenderCommand::DrawIndexedPrimitivesIndirect {
            primitive_type: PrimitiveType::Triangle,
            index_type: IndexType::UInt16,
            index_buffer: indices.clone(),
            index_buffer_offset: index_offset,
            indirect_buffer: buffer,
            indirect_buffer_offset: u64::MIN,
        };
        assert_eq!(
            &drawn_with(&mut subject, &state, draw),
            if draws { &green } else { &clear },
            "16-bit indexed draw of {arguments:?} from byte {index_offset}"
        );
    }
    let texture = target(&mut subject);
    for index_offset in ODD_LENGTH_REFUSED_OFFSETS {
        let buffer = filled(&mut subject, &indexed_draw_bytes(ONE_INDEX_DRAW));
        let draw = RenderCommand::DrawIndexedPrimitivesIndirect {
            primitive_type: PrimitiveType::Triangle,
            index_type: IndexType::UInt16,
            index_buffer: indices.clone(),
            index_buffer_offset: index_offset,
            indirect_buffer: buffer,
            indirect_buffer_offset: u64::MIN,
        };
        refuses_draw(&mut subject, &pass(&texture), &state, vec![draw]);
    }
}

/// An indirect dispatch of one threadgroup.
///
/// https://developer.apple.com/documentation/metal/mtldispatchthreadgroupsindirectarguments
const ONE_GROUP: DispatchThreadgroupsIndirectArguments = groups(1, 1, 1);

/// Checks that one buffer may be a draw's arguments, indices and vertices at once.
///
/// Its words read as positions lie within a pixel of the origin, so the triangle covers no pixel centre. Arguments
/// ending at a buffer's last byte are taken; a buffer shorter than the arguments read is refused.
pub fn draws_with_one_buffer_in_every_role_and_arguments_at_its_end<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let (green, clear) = green_and_clear();
    // The indexed arguments of the covering draw, read as indices 3, 1 and 0 too, then zeros.
    let mut words = indexed_draw_bytes(COVERING_INDEXED_DRAW);
    words.resize(INDIRECT_SPAN as usize, u8::MIN);
    let shared = filled(&mut subject, &words);
    let depth_stencil = no_depth(&mut subject);
    let color = filled(&mut subject, &color_bytes(GREEN));
    let commands = vec![
        RenderCommand::SetRenderPipelineState(state.clone()),
        RenderCommand::SetDepthStencilState(depth_stencil),
        RenderCommand::SetVertexBuffer {
            buffer: shared.clone(),
            offset: u64::MIN,
            length: INDIRECT_SPAN,
            index: VERTICES,
        },
        RenderCommand::SetViewports(vec![TARGET_VIEWPORT]),
        RenderCommand::SetScissorRects(vec![ScissorRect {
            x: u32::MIN,
            y: u32::MIN,
            width: TARGET_SIDE,
            height: TARGET_SIDE,
        }]),
        RenderCommand::SetFragmentBuffer {
            buffer: color,
            offset: u64::MIN,
            length: COLOR_BYTES,
            index: u32::MIN,
        },
        RenderCommand::DrawIndexedPrimitivesIndirect {
            primitive_type: PrimitiveType::Triangle,
            index_type: IndexType::UInt32,
            index_buffer: shared.clone(),
            index_buffer_offset: u64::MIN,
            indirect_buffer: shared.clone(),
            indirect_buffer_offset: u64::MIN,
        },
    ];
    let texture = target(&mut subject);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_render_commands(&command_buffer, &pass(&texture), &commands)
        .expect("Metal answers encode render commands");
    finish(&mut subject, &command_buffer);
    assert_eq!(
        read_texture(&mut subject, &texture),
        clear,
        "one buffer in every role"
    );
    // Arguments ending at the last byte of their buffer are taken; a draw's arguments are short for indexed ones.
    let mut words = vec![u8::MIN; DRAW_ARGUMENTS_BYTES as usize];
    words.extend(indexed_draw_bytes(COVERING_INDEXED_DRAW));
    let edge = filled(&mut subject, &words);
    let indices = filled(&mut subject, &word_bytes(&COVERING_INDICES));
    let draw = RenderCommand::DrawIndexedPrimitivesIndirect {
        primitive_type: PrimitiveType::Triangle,
        index_type: IndexType::UInt32,
        index_buffer: indices.clone(),
        index_buffer_offset: u64::MIN,
        indirect_buffer: edge,
        indirect_buffer_offset: DRAW_ARGUMENTS_BYTES,
    };
    assert_eq!(
        &drawn_with(&mut subject, &state, draw),
        &green,
        "arguments at the buffer's end"
    );
    let short = filled(&mut subject, &draw_bytes(COVERING_DRAW));
    let draw = RenderCommand::DrawIndexedPrimitivesIndirect {
        primitive_type: PrimitiveType::Triangle,
        index_type: IndexType::UInt32,
        index_buffer: indices,
        index_buffer_offset: u64::MIN,
        indirect_buffer: short,
        indirect_buffer_offset: u64::MIN,
    };
    let texture = target(&mut subject);
    refuses_draw(&mut subject, &pass(&texture), &state, vec![draw]);
    // A buffer one word short of a dispatch's arguments.
    let words_buffer = filled(&mut subject, &pattern(WORDS_BYTES as usize));
    let compute = function(&mut subject, naga::ShaderStage::Compute, DOUBLE);
    let compute_state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    let mut short_words = dispatch_bytes(ONE_GROUP);
    short_words.truncate((DISPATCH_ARGUMENTS_BYTES - INDIRECT_ARGUMENTS_ALIGNMENT) as usize);
    let eight = filled(&mut subject, &short_words);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    assert!(matches!(
        subject.encode_compute_commands(
            &command_buffer,
            &[
                ComputeCommand::SetComputePipelineState(compute_state),
                ComputeCommand::SetBuffer {
                    buffer: words_buffer,
                    offset: u64::MIN,
                    length: WORDS_BYTES,
                    index: WORDS_SLOT,
                },
                ComputeCommand::DispatchThreadgroupsIndirect {
                    indirect_buffer: eight,
                    indirect_buffer_offset: u64::MIN,
                    max_threadgroups: u32::MAX,
                },
            ],
        ),
        Err(EncodeComputeCommandsError::OutOfRange(_))
    ));
}

/// The indirect draws of one pass in `draws_a_thousand_indirect_draws_in_one_pass`.
///
/// https://developer.apple.com/documentation/metal/mtldrawprimitivesindirectarguments
const THOUSAND: usize = 1000;

/// Indexed draws of the covering triangle's indices that draw nothing.
///
/// Each reaches past the three indices, or its first instance plus its instances wraps.
/// https://developer.apple.com/documentation/metal/mtldrawindexedprimitivesindirectarguments
const HOSTILE_INDEXED_DRAWS: [DrawIndexedPrimitivesIndirectArguments; 5] = [
    DrawIndexedPrimitivesIndirectArguments {
        index_start: 1,
        ..COVERING_INDEXED_DRAW
    },
    DrawIndexedPrimitivesIndirectArguments {
        index_count: u32::MAX,
        ..COVERING_INDEXED_DRAW
    },
    DrawIndexedPrimitivesIndirectArguments {
        instance_count: 2,
        base_instance: u32::MAX,
        ..COVERING_INDEXED_DRAW
    },
    DrawIndexedPrimitivesIndirectArguments {
        index_count: 2,
        index_start: u32::MAX,
        ..COVERING_INDEXED_DRAW
    },
    DrawIndexedPrimitivesIndirectArguments {
        instance_count: u32::MAX,
        base_instance: 2,
        ..COVERING_INDEXED_DRAW
    },
];

/// Draws without indices that draw nothing.
///
/// In each, the first vertex plus the count, or the first instance plus the instances, wraps.
/// https://developer.apple.com/documentation/metal/mtldrawprimitivesindirectarguments
const HOSTILE_DRAWS: [DrawPrimitivesIndirectArguments; 3] = [
    DrawPrimitivesIndirectArguments {
        vertex_start: u32::MAX,
        ..COVERING_DRAW
    },
    DrawPrimitivesIndirectArguments {
        instance_count: 2,
        base_instance: u32::MAX,
        ..COVERING_DRAW
    },
    DrawPrimitivesIndirectArguments {
        vertex_count: u32::MAX,
        vertex_start: 1,
        ..COVERING_DRAW
    },
];

/// Checks a thousand indirect draws in one pass, each at its own offset of one buffer, indexed and not in turn.
///
/// Every one whose counts wrap or reach past its indices draws nothing, and a good last one still draws.
pub fn draws_a_thousand_indirect_draws_in_one_pass<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let (green, clear) = green_and_clear();
    let indices = filled(&mut subject, &word_bytes(&COVERING_INDICES));
    for last_is_good in [true, false] {
        let mut words = Vec::new();
        for k in 0..THOUSAND {
            let mut slot = if k == THOUSAND - 1 && last_is_good {
                indexed_draw_bytes(COVERING_INDEXED_DRAW)
            } else if k % 2 == 0 {
                indexed_draw_bytes(HOSTILE_INDEXED_DRAWS[k % HOSTILE_INDEXED_DRAWS.len()])
            } else {
                draw_bytes(HOSTILE_DRAWS[k % HOSTILE_DRAWS.len()])
            };
            // Every slot is as long as an indexed draw's arguments; a draw without indices reads the first of them.
            slot.resize(DRAW_INDEXED_ARGUMENTS_BYTES as usize, u8::MIN);
            words.extend(slot);
        }
        let arguments = filled(&mut subject, &words);
        let draws: Vec<RenderCommand<M>> = (0..THOUSAND)
            .map(|k| {
                let at = k as u64 * DRAW_INDEXED_ARGUMENTS_BYTES;
                if k % 2 == 0 {
                    RenderCommand::DrawIndexedPrimitivesIndirect {
                        primitive_type: PrimitiveType::Triangle,
                        index_type: IndexType::UInt32,
                        index_buffer: indices.clone(),
                        index_buffer_offset: u64::MIN,
                        indirect_buffer: arguments.clone(),
                        indirect_buffer_offset: at,
                    }
                } else {
                    RenderCommand::DrawPrimitivesIndirect {
                        primitive_type: PrimitiveType::Triangle,
                        indirect_buffer: arguments.clone(),
                        indirect_buffer_offset: at,
                    }
                }
            })
            .collect();
        let seen = drawn_with_all(&mut subject, &state, draws);
        if last_is_good {
            assert_eq!(&seen, &green, "the good last draw");
        } else {
            assert_eq!(&seen, &clear, "a thousand hostile draws");
        }
    }
}

/// Checks that a draw without indices of the largest count a `u32` holds from vertex 0 is drawn.
///
/// It draws the covering triangle, its vertices past the three bound reading zeros.
pub fn draws_the_largest_count_from_vertex_zero<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, FLAT);
    let (green, _) = green_and_clear();
    let largest = DrawPrimitivesIndirectArguments {
        vertex_count: u32::MAX,
        ..COVERING_DRAW
    };
    let buffer = filled(&mut subject, &draw_bytes(largest));
    let draw = RenderCommand::DrawPrimitivesIndirect {
        primitive_type: PrimitiveType::Triangle,
        indirect_buffer: buffer,
        indirect_buffer_offset: u64::MIN,
    };
    let start = std::time::Instant::now();
    let seen = drawn_with(&mut subject, &state, draw);
    eprintln!("u32::MAX vertices: {} ms", start.elapsed().as_millis());
    assert_eq!(seen, green);
}

/// The compute function that steps a generator `words[1]` times, to keep the GPU busy for a while.
const SPIN: &str = "spin";

/// Encodes a dispatch of one thread of `SPIN` reading `words` into `command_buffer`.
fn spin<M: Metal>(subject: &mut M, command_buffer: &M::CommandBuffer, words: &M::Buffer) {
    let compute = function(subject, naga::ShaderStage::Compute, SPIN);
    let state = subject
        .make_compute_pipeline_state(&compute)
        .expect("Metal answers make compute pipeline state");
    subject
        .encode_compute_commands(
            command_buffer,
            &[
                ComputeCommand::SetComputePipelineState(state),
                ComputeCommand::SetBuffer {
                    buffer: words.clone(),
                    offset: u64::MIN,
                    length: WORDS_BYTES,
                    index: WORDS_SLOT,
                },
                ComputeCommand::DispatchThreadgroups(Size {
                    width: SINGLE,
                    height: SINGLE,
                    depth: SINGLE,
                }),
            ],
        )
        .expect("Metal answers encode compute commands");
}

/// The steps of `SPIN` that keep one GPU thread busy for tens of milliseconds.
///
/// https://developer.apple.com/documentation/metal/mtlcommandbuffer/gpustarttime
pub const SPIN_STEPS: u32 = 1 << 23;

/// The words `spin` reads: the seed it starts from, then the steps it takes, then the words it writes.
///
/// https://www.w3.org/TR/WGSL/#for-statement
const SPIN_WORDS: [u32; WORD_COUNT] = [1, SPIN_STEPS, 0, 0, 0, 0, 0, 0];

/// Checks that timestamps written around work hold the GPU's clock in nanoseconds.
///
/// They are in order, and apart by about the time the CPU waited for the work, which runs for tens of milliseconds.
pub fn writes_the_gpu_clock_in_nanoseconds<M: Metal>(mut subject: M) {
    let Ok(timestamps) = subject.make_timestamp_buffer(3) else {
        return;
    };
    let words = filled(&mut subject, &word_bytes(&SPIN_WORDS));
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_timestamp(&command_buffer, &timestamps, u32::MIN)
        .expect("Metal answers encode timestamp");
    spin(&mut subject, &command_buffer, &words);
    subject
        .encode_timestamp(&command_buffer, &timestamps, 1)
        .expect("Metal answers encode timestamp");
    subject
        .encode_timestamp(&command_buffer, &timestamps, 2)
        .expect("Metal answers encode timestamp");
    let mut read = [u64::MIN; 3];
    assert_eq!(
        subject.read_timestamps(&timestamps, 0, &mut read),
        Err(wcontainers_metal::ReadTimestampsError::UsedByUncommittedWork)
    );
    let start = std::time::Instant::now();
    finish(&mut subject, &command_buffer);
    let waited = start.elapsed().as_nanos() as f64;
    subject
        .read_timestamps(&timestamps, 0, &mut read)
        .expect("Metal answers read timestamps");
    let [t0, t1, t2] = read;
    eprintln!("timestamps: {t0} {t1} {t2}, {waited} ns waited");
    assert!(
        t0 != u64::MAX && t0 != 0 && t0 <= t1 && t1 <= t2,
        "{t0} {t1} {t2}"
    );
    let span = (t1 - t0) as f64;
    assert!(
        span <= waited * 1.05 && span >= waited * 0.25,
        "{span} ns of GPU work in {waited} ns"
    );
}

/// The places of the timestamp buffer the range tests make.
///
/// https://developer.apple.com/documentation/metal/mtlcountersamplebufferdescriptor/samplecount
const TIMESTAMP_PLACES: u32 = 4;

/// Reads of `TIMESTAMP_PLACES` places that are refused, as a first place and a count: each reaches past the last.
///
/// https://developer.apple.com/documentation/metal/mtlcountersamplebuffer/resolvecounterrange(_:)
const REFUSED_TIMESTAMP_READS: [(u32, usize); 6] =
    [(0, 5), (4, 1), (3, 2), (5, 0), (u32::MAX, 2), (1, 4)];

/// Checks that timestamp buffers of no places or past the most, and reads past the last place, are refused.
pub fn refuses_timestamps_outside_their_buffers<M: Metal>(mut subject: M) {
    for count in [u32::MIN, wcontainers_metal::MAX_TIMESTAMPS + 1, u32::MAX] {
        assert_eq!(
            subject.make_timestamp_buffer(count).err(),
            Some(wcontainers_metal::MakeTimestampBufferError::Count)
        );
    }
    let Ok(timestamps) = subject.make_timestamp_buffer(TIMESTAMP_PLACES) else {
        return;
    };
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    for index in [TIMESTAMP_PLACES, TIMESTAMP_PLACES + 1, u32::MAX] {
        assert_eq!(
            subject.encode_timestamp(&command_buffer, &timestamps, index),
            Err(wcontainers_metal::EncodeTimestampError::OutOfRange)
        );
    }
    for (first, count) in REFUSED_TIMESTAMP_READS {
        let mut into = vec![u64::MIN; count];
        assert_eq!(
            subject.read_timestamps(&timestamps, first, &mut into),
            Err(wcontainers_metal::ReadTimestampsError::OutOfRange),
            "a read of {count} from {first}"
        );
    }
    let mut none = [u64::MIN; 0];
    subject
        .read_timestamps(&timestamps, TIMESTAMP_PLACES, &mut none)
        .expect("Metal answers read timestamps");
    subject
        .commit(&command_buffer)
        .expect("Metal answers commit");
    assert_eq!(
        subject.encode_timestamp(&command_buffer, &timestamps, u32::MIN),
        Err(wcontainers_metal::EncodeTimestampError::Committed)
    );
}

/// Checks that a timestamp buffer dropped after its timestamps are encoded is kept until its work completes.
pub fn keeps_a_timestamp_buffer_dropped_before_its_work_runs<M: Metal>(mut subject: M) {
    let Ok(timestamps) = subject.make_timestamp_buffer(2) else {
        return;
    };
    let words = filled(&mut subject, &word_bytes(&SPIN_WORDS));
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_timestamp(&command_buffer, &timestamps, u32::MIN)
        .expect("Metal answers encode timestamp");
    spin(&mut subject, &command_buffer, &words);
    subject
        .encode_timestamp(&command_buffer, &timestamps, 1)
        .expect("Metal answers encode timestamp");
    drop(timestamps);
    finish(&mut subject, &command_buffer);
}

/// Checks that a read of timestamps waits for committed work that writes them after its handle was dropped.
///
/// The read is refused while an uncommitted writer is held, and taken once that writer is dropped.
pub fn waits_for_a_dropped_writer_before_reading_timestamps<M: Metal>(mut subject: M) {
    let Ok(timestamps) = subject.make_timestamp_buffer(2) else {
        return;
    };
    let words = filled(&mut subject, &word_bytes(&SPIN_WORDS));
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_timestamp(&command_buffer, &timestamps, u32::MIN)
        .expect("Metal answers encode timestamp");
    spin(&mut subject, &command_buffer, &words);
    subject
        .encode_timestamp(&command_buffer, &timestamps, 1)
        .expect("Metal answers encode timestamp");
    subject
        .commit(&command_buffer)
        .expect("Metal answers commit");
    drop(command_buffer);
    let mut read = [u64::MIN; 2];
    subject
        .read_timestamps(&timestamps, 0, &mut read)
        .expect("Metal answers read timestamps");
    // The spin runs for tens of milliseconds, so the two places are more than a millisecond apart once it ended.
    assert!(
        read[0] != 0 && read[0] <= read[1] && read[1] - read[0] > 1_000_000,
        "{read:?}"
    );
    let second = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_timestamp(&second, &timestamps, 1)
        .expect("Metal answers encode timestamp");
    assert_eq!(
        subject.read_timestamps(&timestamps, 0, &mut read),
        Err(wcontainers_metal::ReadTimestampsError::UsedByUncommittedWork)
    );
    drop(second);
    subject
        .read_timestamps(&timestamps, 0, &mut read)
        .expect("Metal answers read timestamps");
}

/// Checks a buffer of the most places, written at its first place and twice at its last, then read whole and at its last.
pub fn writes_and_reads_timestamps_at_the_most_places<M: Metal>(mut subject: M) {
    let most = wcontainers_metal::MAX_TIMESTAMPS;
    let Ok(timestamps) = subject.make_timestamp_buffer(most) else {
        return;
    };
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    for index in [0, most - 1, most - 1] {
        subject
            .encode_timestamp(&command_buffer, &timestamps, index)
            .expect("Metal answers encode timestamp");
    }
    finish(&mut subject, &command_buffer);
    let mut read = vec![u64::MIN; most as usize];
    subject
        .read_timestamps(&timestamps, 0, &mut read)
        .expect("Metal answers read timestamps");
    let last = read[most as usize - 1];
    assert!(read[0] != 0 && read[0] <= last, "{} {last}", read[0]);
    let mut one = [u64::MIN; 1];
    subject
        .read_timestamps(&timestamps, most - 1, &mut one)
        .expect("Metal answers read timestamps");
    assert_eq!(one[0], last);
}

/// Functions that ask a texture its levels and then reach a thousand levels past them.
const QUERIES: &str = "
@group(0) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn size_past() -> @location(0) vec4<f32> {
    let n = textureNumLevels(t);
    let d = textureDimensions(t, n + 1000u);
    return vec4<f32>(f32(d.x) / 255.0, f32(d.y) / 255.0, f32(n) / 255.0, 1.0);
}
@fragment fn load_past() -> @location(0) vec4<f32> {
    let n = textureNumLevels(t);
    return textureLoad(t, vec2<i32>(0, 0), i32(n) + 1000);
}
";

/// The levels of the textures `QUERIES` reads: four by four texels down to one.
///
/// https://developer.apple.com/documentation/metal/mtltexturedescriptor/mipmaplevelcount
const QUERIED_LEVELS: u32 = 3;

/// Compiles one entry point of `QUERIES`.
fn query_function<M: Metal>(
    subject: &mut M,
    stage: naga::ShaderStage,
    entry_point: &str,
) -> M::Function {
    let module = naga::front::wgsl::parse_str(QUERIES).expect("the queries parse");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the queries validate");
    let vertex_buffers = [VertexBufferLayout {
        buffer_index: VERTICES,
        stride: 8,
        step_function: VertexStepFunction::PerVertex,
        step_rate: 1,
        attributes: vec![VertexAttribute {
            location: u32::MIN,
            offset: u32::MIN,
            format: VertexFormat::Float2,
        }],
    }];
    subject
        .make_function(&FunctionSource {
            module: &module,
            info: &info,
            stage,
            entry_point,
            resources: &SLOTS[2..3],
            immediates_buffer_index: IMMEDIATES,
            sizes_buffer_index: SIZES,
            vertex_buffers: &vertex_buffers,
            point_size: false,
        })
        .expect("Metal answers make function")
}

/// Draws with the `QUERIES` fragment function `fragment` reading `texture` at slot 0.
///
/// Returns the first texel drawn or the encode's refusal.
fn query_drawn<M: Metal>(
    subject: &mut M,
    fragment: &str,
    texture: &M::Texture,
) -> Result<Vec<u8>, EncodeRenderCommandsError> {
    let vertex = query_function(subject, naga::ShaderStage::Vertex, VERTEX);
    let fragment = query_function(subject, naga::ShaderStage::Fragment, fragment);
    let mut color_attachments = [None; MAX_COLOR_ATTACHMENTS];
    color_attachments[0] = Some(RenderPipelineColorAttachmentDescriptor {
        pixel_format: PixelFormat::Rgba8Unorm,
        blending: None,
        write_mask: ColorWriteMask {
            red: true,
            green: true,
            blue: true,
            alpha: true,
        },
    });
    let state = subject
        .make_render_pipeline_state(&RenderPipelineDescriptor {
            vertex_function: &vertex,
            fragment_function: Some(&fragment),
            color_attachments,
            depth_attachment_pixel_format: None,
            stencil_attachment_pixel_format: None,
            raster_sample_count: SINGLE,
            alpha_to_coverage_enabled: false,
            rasterization_enabled: true,
            input_primitive_topology: PrimitiveTopologyClass::Triangle,
        })
        .expect("Metal answers make render pipeline state");
    let depth_stencil = no_depth(subject);
    let vertices = covering(subject);
    let drawn = target(subject);
    let bound = vec![RenderCommand::SetFragmentTexture {
        texture: texture.clone(),
        index: u32::MIN,
    }];
    let commands = covering_draw(&state, &vertices, &depth_stencil, bound);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject.encode_render_commands(&command_buffer, &pass(&drawn), &commands)?;
    finish(subject, &command_buffer);
    Ok(read_texture(subject, &drawn)[..RGBA8_BYTES as usize].to_vec())
}

/// Makes a sampled `rgba8Unorm` texture of `QUERIED_LEVELS` levels whose last level's one texel holds `texel`.
fn levels_texture<M: Metal>(
    subject: &mut M,
    texture_type: TextureType,
    array_length: u32,
    texel: [u8; 4],
) -> M::Texture {
    let texture = subject
        .make_texture(&TextureDescriptor {
            texture_type,
            pixel_format: PixelFormat::Rgba8Unorm,
            width: TARGET_SIDE,
            height: TARGET_SIDE,
            depth: SINGLE,
            mipmap_level_count: QUERIED_LEVELS,
            sample_count: SINGLE,
            array_length,
            usage: TextureUsage {
                shader_read: true,
                shader_write: false,
                render_target: true,
                pixel_format_view: true,
            },
        })
        .expect("Metal answers make texture");
    let source = filled(subject, &texel);
    let command_buffer = subject
        .make_command_buffer()
        .expect("Metal answers make command buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::CopyFromBufferToTexture {
                source_buffer: source,
                source_offset: u64::MIN,
                source_bytes_per_row: RGBA8_BYTES,
                source_bytes_per_image: RGBA8_BYTES,
                source_size: Size {
                    width: SINGLE,
                    height: SINGLE,
                    depth: SINGLE,
                },
                destination_texture: texture.clone(),
                destination_slice: u32::MIN,
                destination_level: QUERIED_LEVELS - 1,
                destination_origin: Origin::default(),
                options: BlitOption::None,
            }],
        )
        .expect("Metal answers encode blit commands");
    finish(subject, &command_buffer);
    texture
}

/// A texel no cleared target holds.
///
/// https://developer.apple.com/documentation/metal/mtlpixelformat/rgba8unorm
const LAST_LEVEL_TEXEL: [u8; 4] = [10, 20, 30, 40];

/// Checks that a size query of a level a thousand past the last completes.
///
/// naga writes it as Metal's `get_width(lod)` with no clamp; of a texture and of a one-level view, beside the level
/// count it draws.
pub fn answers_a_size_query_of_a_level_past_the_last<M: Metal>(mut subject: M) {
    let texture = levels_texture(&mut subject, TextureType::Type2D, SINGLE, LAST_LEVEL_TEXEL);
    let seen = query_drawn(&mut subject, "size_past", &texture).expect("a query past the levels");
    eprintln!("size of level n + 1000: {seen:?}");
    assert_eq!(seen[2], QUERIED_LEVELS as u8);
    let view = subject
        .make_texture_view(
            &texture,
            &TextureViewDescriptor {
                pixel_format: PixelFormat::Rgba8Unorm,
                texture_type: TextureType::Type2D,
                first_level: QUERIED_LEVELS - 1,
                level_count: SINGLE,
                first_slice: u32::MIN,
                slice_count: SINGLE,
                swizzle: TextureSwizzleChannels::IDENTITY,
            },
        )
        .expect("Metal answers make texture view");
    let seen = query_drawn(&mut subject, "size_past", &view).expect("a query past the levels");
    eprintln!("size of level n + 1000 of a one-level view: {seen:?}");
    assert_eq!(seen[2], SINGLE as u8);
}

/// Checks that a load a thousand levels past the level count reads the last level, as naga's restricted loads clamp it.
pub fn loads_the_last_level_for_a_level_past_it<M: Metal>(mut subject: M) {
    let texture = levels_texture(&mut subject, TextureType::Type2D, SINGLE, LAST_LEVEL_TEXEL);
    let seen = query_drawn(&mut subject, "load_past", &texture).expect("a load past the levels");
    assert_eq!(seen, LAST_LEVEL_TEXEL);
}

/// Checks that a 2D array texture at a queried function's 2D texture slot is refused before Metal.
pub fn refuses_a_texture_of_another_type_at_a_queried_slot<M: Metal>(mut subject: M) {
    let texture = levels_texture(&mut subject, TextureType::Type2DArray, 2, LAST_LEVEL_TEXEL);
    let seen = query_drawn(&mut subject, "size_past", &texture);
    assert!(
        matches!(seen, Err(EncodeRenderCommandsError::Unbound(_))),
        "{seen:?}"
    );
}

/// Runs each suite function above as its own test on a value made anew for every test.
#[macro_export]
macro_rules! tests {
    (@StandaloneMetal yes ($value:expr)) => {
        $crate::tests!(@StandaloneMetal yes ($value) ($value));
    };
    (@StandaloneMetal yes ($base:expr) ($value:expr)) => {
        mod standalone_metal {}
    };
    (@StandaloneMetal no ($value:expr)) => {
        mod standalone_metal {
            #[test]
            fn lacks_standalone_metal() {
                struct Probe<T>(::core::marker::PhantomData<T>);
                impl<T> ::core::clone::Clone for Probe<T> {
                    fn clone(&self) -> Self {
                        Probe(::core::marker::PhantomData)
                    }
                }
                struct Implemented;
                #[allow(dead_code)]
                impl<T: ::wcontainers_metal::StandaloneMetal> Probe<T> {
                    fn clone(&self) -> Implemented {
                        Implemented
                    }
                }
                fn probe_of<T>(_value: &T) -> Probe<T> {
                    Probe(::core::marker::PhantomData)
                }
                let _: Probe<_> = probe_of(&$value).clone();
            }
        }
    };
    ($value:expr; StandaloneMetal: $standalone_metal:ident $(($standalone_metal_value:expr))?) => {
        #[test]
        fn has_a_name() {
            $crate::has_a_name($value);
        }

        #[test]
        fn steps_instance_buffers_by_their_rate() {
            $crate::steps_instance_buffers_by_their_rate($value);
        }

        #[test]
        fn writes_the_gpu_clock_in_nanoseconds() {
            $crate::writes_the_gpu_clock_in_nanoseconds($value);
        }

        #[test]
        fn refuses_timestamps_outside_their_buffers() {
            $crate::refuses_timestamps_outside_their_buffers($value);
        }

        #[test]
        fn draws_with_counts_the_gpu_reads() {
            $crate::draws_with_counts_the_gpu_reads($value);
        }

        #[test]
        fn draws_with_counts_written_before_them_and_waits_for_writes() {
            $crate::draws_with_counts_written_before_them_and_waits_for_writes($value);
        }

        #[test]
        fn dispatches_threadgroups_the_gpu_reads() {
            $crate::dispatches_threadgroups_the_gpu_reads($value);
        }

        #[test]
        fn refuses_indirect_arguments_outside_their_buffer() {
            $crate::refuses_indirect_arguments_outside_their_buffer($value);
        }

        #[test]
        fn draws_only_whole_16_bit_indices_of_an_odd_length_buffer() {
            $crate::draws_only_whole_16_bit_indices_of_an_odd_length_buffer($value);
        }

        #[test]
        fn draws_with_one_buffer_in_every_role_and_arguments_at_its_end() {
            $crate::draws_with_one_buffer_in_every_role_and_arguments_at_its_end($value);
        }

        #[test]
        fn draws_a_thousand_indirect_draws_in_one_pass() {
            $crate::draws_a_thousand_indirect_draws_in_one_pass($value);
        }

        #[test]
        fn draws_the_largest_count_from_vertex_zero() {
            $crate::draws_the_largest_count_from_vertex_zero($value);
        }

        #[test]
        fn keeps_a_timestamp_buffer_dropped_before_its_work_runs() {
            $crate::keeps_a_timestamp_buffer_dropped_before_its_work_runs($value);
        }

        #[test]
        fn waits_for_a_dropped_writer_before_reading_timestamps() {
            $crate::waits_for_a_dropped_writer_before_reading_timestamps($value);
        }

        #[test]
        fn writes_and_reads_timestamps_at_the_most_places() {
            $crate::writes_and_reads_timestamps_at_the_most_places($value);
        }

        #[test]
        fn answers_a_size_query_of_a_level_past_the_last() {
            $crate::answers_a_size_query_of_a_level_past_the_last($value);
        }

        #[test]
        fn loads_the_last_level_for_a_level_past_it() {
            $crate::loads_the_last_level_for_a_level_past_it($value);
        }

        #[test]
        fn refuses_a_texture_of_another_type_at_a_queried_slot() {
            $crate::refuses_a_texture_of_another_type_at_a_queried_slot($value);
        }

        #[test]
        fn samples_abgr4_with_alpha_in_its_low_bits() {
            $crate::samples_abgr4_with_alpha_in_its_low_bits($value);
        }

        #[test]
        fn samples_a_swizzled_view() {
            $crate::samples_a_swizzled_view($value);
        }

        #[test]
        fn refuses_a_swizzle_of_a_texture_drawn_into() {
            $crate::refuses_a_swizzle_of_a_texture_drawn_into($value);
        }

        #[test]
        fn discards_fragments_whose_stored_depth_is_outside_the_bounds() {
            $crate::discards_fragments_whose_stored_depth_is_outside_the_bounds($value);
        }

        #[test]
        fn builds_functions_and_pipelines_in_the_background() {
            $crate::builds_functions_and_pipelines_in_the_background($value);
        }

        #[test]
        fn writes_and_reads_buffer_contents() {
            $crate::writes_and_reads_buffer_contents($value);
        }

        #[test]
        fn refuses_contents_of_uncommitted_work_and_a_second_commit() {
            $crate::refuses_contents_of_uncommitted_work_and_a_second_commit($value);
        }

        #[test]
        fn frees_a_buffer_whose_command_buffer_was_dropped() {
            $crate::frees_a_buffer_whose_command_buffer_was_dropped($value);
        }

        #[test]
        fn copies_and_fills_move_exact_bytes() {
            $crate::copies_and_fills_move_exact_bytes($value);
        }

        #[test]
        fn draws_in_a_uniform_color_and_refuses_an_empty_slot() {
            $crate::draws_in_a_uniform_color_and_refuses_an_empty_slot($value);
        }

        #[test]
        fn samples_a_texture_view() {
            $crate::samples_a_texture_view($value);
        }

        #[test]
        fn dispatches_a_compute_function() {
            $crate::dispatches_a_compute_function($value);
        }

        #[test]
        fn dispatches_with_workgroup_memory() {
            $crate::dispatches_with_workgroup_memory($value);
        }

        #[test]
        fn refuses_a_texture_wider_than_any_gpu_makes() {
            $crate::refuses_a_texture_wider_than_any_gpu_makes($value);
        }

        #[test]
        fn refuses_a_3d_texture_deeper_than_the_most() {
            $crate::refuses_a_3d_texture_deeper_than_the_most($value);
        }

        #[test]
        fn refuses_more_layers_than_the_most() {
            $crate::refuses_more_layers_than_the_most($value);
        }

        #[test]
        fn refuses_a_texture_in_a_view_only_format() {
            $crate::refuses_a_texture_in_a_view_only_format($value);
        }

        #[test]
        fn refuses_a_stencil_view_of_a_texture_not_made_for_views() {
            $crate::refuses_a_stencil_view_of_a_texture_not_made_for_views($value);
        }

        #[test]
        fn refuses_a_pipeline_drawing_into_a_compressed_format() {
            $crate::refuses_a_pipeline_drawing_into_a_compressed_format($value);
        }

        #[test]
        fn refuses_a_pipeline_blending_an_integer_format() {
            $crate::refuses_a_pipeline_blending_an_integer_format($value);
        }

        #[test]
        fn refuses_a_pipeline_of_mismatched_depth_and_stencil() {
            $crate::refuses_a_pipeline_of_mismatched_depth_and_stencil($value);
        }

        #[test]
        fn refuses_a_pass_drawing_into_a_compressed_texture() {
            $crate::refuses_a_pass_drawing_into_a_compressed_texture($value);
        }

        #[test]
        fn refuses_a_depth_clear_outside_zero_to_one() {
            $crate::refuses_a_depth_clear_outside_zero_to_one($value);
        }

        #[test]
        fn refuses_a_resolve_metal_does_not_make() {
            $crate::refuses_a_resolve_metal_does_not_make($value);
        }

        #[test]
        fn refuses_a_viewport_that_is_not_finite() {
            $crate::refuses_a_viewport_that_is_not_finite($value);
        }

        #[test]
        fn refuses_a_uniform_at_a_misaligned_offset() {
            $crate::refuses_a_uniform_at_a_misaligned_offset($value);
        }

        #[test]
        fn refuses_draws_of_no_instances_or_indices() {
            $crate::refuses_draws_of_no_instances_or_indices($value);
        }

        #[test]
        fn refuses_a_texture_not_made_to_be_read() {
            $crate::refuses_a_texture_not_made_to_be_read($value);
        }

        #[test]
        fn refuses_a_visibility_offset_past_the_span() {
            $crate::refuses_a_visibility_offset_past_the_span($value);
        }

        #[test]
        fn refuses_depth_and_stencil_tests_without_attachments() {
            $crate::refuses_depth_and_stencil_tests_without_attachments($value);
        }

        #[test]
        fn refuses_empty_dispatches_and_misaligned_words() {
            $crate::refuses_empty_dispatches_and_misaligned_words($value);
        }

        #[test]
        fn refuses_buffer_copies_and_fills_metal_does_not_make() {
            $crate::refuses_buffer_copies_and_fills_metal_does_not_make($value);
        }

        #[test]
        fn refuses_copy_pitches_metal_does_not_take() {
            $crate::refuses_copy_pitches_metal_does_not_take($value);
        }

        #[test]
        fn refuses_a_copy_between_texture_formats_that_differ() {
            $crate::refuses_a_copy_between_texture_formats_that_differ($value);
        }

        #[test]
        fn refuses_counting_twice_at_one_offset() {
            $crate::refuses_counting_twice_at_one_offset($value);
        }

        #[test]
        fn adds_occlusion_counts_across_passes() {
            $crate::adds_occlusion_counts_across_passes($value);
        }

        #[test]
        fn refuses_mipmaps_of_one_level() {
            $crate::refuses_mipmaps_of_one_level($value);
        }

        #[test]
        fn refuses_every_range_whose_end_wraps() {
            $crate::refuses_every_range_whose_end_wraps($value);
        }

        #[test]
        fn refuses_alpha_to_coverage_into_an_integer_format() {
            $crate::refuses_alpha_to_coverage_into_an_integer_format($value);
        }

        #[test]
        fn refuses_a_sampler_clamping_both_to_zero_and_to_the_border() {
            $crate::refuses_a_sampler_clamping_both_to_zero_and_to_the_border($value);
        }

        #[test]
        fn refuses_one_texture_as_two_attachments() {
            $crate::refuses_one_texture_as_two_attachments($value);
        }

        #[test]
        fn refuses_an_empty_byte_write_at_a_slot_a_function_reads() {
            $crate::refuses_an_empty_byte_write_at_a_slot_a_function_reads($value);
        }

        #[test]
        fn refuses_an_integer_texture_at_a_float_slot() {
            $crate::refuses_an_integer_texture_at_a_float_slot($value);
        }

        #[test]
        fn refuses_a_runtime_array_shorter_than_one_element() {
            $crate::refuses_a_runtime_array_shorter_than_one_element($value);
        }

        #[test]
        fn refuses_a_buffer_past_the_max_buffer_length() {
            $crate::refuses_a_buffer_past_the_max_buffer_length($value);
        }

        #[test]
        fn refuses_a_grid_of_more_threads_on_one_axis_than_a_u32_holds() {
            $crate::refuses_a_grid_of_more_threads_on_one_axis_than_a_u32_holds($value);
        }

        #[test]
        fn refuses_a_cube_of_more_than_one_slice() {
            $crate::refuses_a_cube_of_more_than_one_slice($value);
        }

        #[test]
        fn refuses_a_second_source_blend_beside_another_attachment() {
            $crate::refuses_a_second_source_blend_beside_another_attachment($value);
        }

        #[test]
        fn refuses_some_channels_of_a_shared_exponent_target() {
            $crate::refuses_some_channels_of_a_shared_exponent_target($value);
        }

        #[test]
        fn refuses_bytes_at_a_slot_the_function_writes() {
            $crate::refuses_bytes_at_a_slot_the_function_writes($value);
        }

        #[test]
        fn refuses_a_vertex_stride_past_the_most() {
            $crate::refuses_a_vertex_stride_past_the_most($value);
        }

        #[test]
        fn refuses_a_visibility_offset_with_no_buffer() {
            $crate::refuses_a_visibility_offset_with_no_buffer($value);
        }

        #[test]
        fn refuses_a_copy_past_a_compressed_levels_texels() {
            $crate::refuses_a_copy_past_a_compressed_levels_texels($value);
        }

        #[test]
        fn refuses_a_pass_of_no_attachment_past_the_longest_side() {
            $crate::refuses_a_pass_of_no_attachment_past_the_longest_side($value);
        }

        #[test]
        fn draws_from_a_vertex_buffer_shorter_than_one_element() {
            $crate::draws_from_a_vertex_buffer_shorter_than_one_element($value);
        }
        $crate::tests!(@StandaloneMetal $standalone_metal ($value) $(($standalone_metal_value))?);
    };
}
