//! The tests every implementation of wcontainers-metal runs, written against its traits alone.

use wcontainers_metal::{
    BlendFactor, BlendOperation, Blending, BlitCommand, BlitOption, ColorWriteMask,
    CommandBufferStatus, CommitError, CompareFunction, ComputeCommand, DepthStencilDescriptor,
    EncodeBlitCommandsError, EncodeComputeCommandsError, EncodeRenderCommandsError, FunctionSource,
    IndexType, LoadAction, MAX_COLOR_ATTACHMENTS, MAX_TEXTURE_SIDE, MAX_VERTEX_STRIDE,
    MakeBufferError, MakeRenderPipelineStateError, MakeSamplerStateError, MakeTextureError, Metal,
    Origin, PixelFormat, PrimitiveTopologyClass, PrimitiveType, ReadContentsError, RenderCommand,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipelineColorAttachmentDescriptor, RenderPipelineDescriptor, ResolveTarget, ResourceSlot,
    SamplerAddressMode, SamplerBorderColor, SamplerDescriptor, SamplerMinMagFilter,
    SamplerMipFilter, ScissorRect, Size, StencilDescriptor, StencilOperation, StoreAction,
    TextureDescriptor, TextureType, TextureUsage, TextureViewDescriptor, VISIBILITY_RESULT_BYTES,
    VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepFunction, Viewport,
    VisibilityResultMode, WaitUntilCompletedError, WriteContentsError,
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

/// The vertices of one triangle.
///
/// https://developer.apple.com/documentation/metal/mtlprimitivetype/triangle
const TRIANGLE_VERTICES: u32 = 3;

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
const COVERING: &[f32] = &[-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];

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
type Color = [f32; 4];

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
        .write_contents(&buffer, 0, bytes)
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
    let mut bytes = vec![0; (row * u64::from(TARGET_SIDE)) as usize];
    subject
        .read_contents(&buffer, 0, &mut bytes)
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
        RenderCommand::SetViewports(vec![Viewport {
            origin_x: 0.0,
            origin_y: 0.0,
            width: f64::from(TARGET_SIDE),
            height: f64::from(TARGET_SIDE),
            znear: 0.0,
            zfar: 1.0,
        }]),
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
        vertex_count: TRIANGLE_VERTICES,
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

/// Checks that bytes written into a buffer read back, and that ranges past its end are refused.
pub fn writes_and_reads_buffer_contents<M: Metal>(mut subject: M) {
    let bytes = pattern(64);
    let buffer = filled(&mut subject, &bytes);
    subject
        .write_contents(&buffer, 3, &[1, 2, 3])
        .expect("Metal answers write contents");
    let mut expected = bytes.clone();
    expected[3..6].copy_from_slice(&[1, 2, 3]);
    let mut read = vec![0; 64];
    subject
        .read_contents(&buffer, 0, &mut read)
        .expect("Metal answers read contents");
    assert_eq!(read, expected);
    assert_eq!(
        subject.write_contents(&buffer, 62, &[0; 3]),
        Err(WriteContentsError::OutOfRange)
    );
    assert_eq!(
        subject.read_contents(&buffer, 65, &mut [0; 1]),
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
        subject.write_contents(&buffer, 0, &[0]),
        Err(WriteContentsError::UsedByUncommittedWork)
    );
    finish(&mut subject, &command_buffer);
    assert_eq!(
        subject.commit(&command_buffer),
        Err(CommitError::AlreadyCommitted)
    );
    let mut read = [0; 4];
    subject
        .read_contents(&buffer, 0, &mut read)
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
        .write_contents(&buffer, 0, &[0; COLOR_BYTES as usize])
        .expect("Metal answers write contents");
}

/// Checks that copies between buffers and a texture, and a fill, move exactly the bytes they name.
pub fn copies_and_fills_move_exact_bytes<M: Metal>(mut subject: M) {
    let row = u64::from(TARGET_SIDE) * RGBA8_BYTES;
    let bytes = pattern((row * u64::from(TARGET_SIDE)) as usize);
    let source = filled(&mut subject, &bytes);
    let texture = target(&mut subject);
    let copy = filled(&mut subject, &[0; 32]);
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
                    source_offset: 4,
                    destination_buffer: copy.clone(),
                    destination_offset: 8,
                    size: 8,
                },
                BlitCommand::FillBuffer {
                    buffer: copy.clone(),
                    offset: 20,
                    length: 4,
                    value: 0xab,
                },
            ],
        )
        .expect("Metal answers encode blit commands");
    finish(&mut subject, &command_buffer);
    assert_eq!(read_texture(&mut subject, &texture), bytes);
    let mut expected = vec![0; 32];
    expected[8..16].copy_from_slice(&bytes[4..12]);
    expected[20..24].fill(0xab);
    let mut read = vec![0; 32];
    subject
        .read_contents(&copy, 0, &mut read)
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
        [0, 255, 0, 255].repeat((TARGET_SIDE * TARGET_SIDE) as usize)
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
    let mut read = vec![0; 32];
    subject
        .read_contents(&buffer, 0, &mut read)
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
    let mut read = [0; 4];
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
    let bytes: Vec<u8> = COVERING.iter().flat_map(|v| v.to_le_bytes()).collect();
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
    let color = filled(subject, &[0; COLOR_BYTES as usize]);
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
        origin_y: 0.0,
        width: f64::from(TARGET_SIDE),
        height: f64::from(TARGET_SIDE),
        znear: 0.0,
        zfar: 1.0,
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
    let color = filled(&mut subject, &[0; (COLOR_BYTES * 2) as usize]);
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
    let indices = filled(&mut subject, &[0; COVERING_BYTES as usize]);
    for draw in [
        RenderCommand::DrawPrimitives {
            primitive_type: PrimitiveType::Triangle,
            vertex_start: u32::MIN,
            vertex_count: TRIANGLE_VERTICES,
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

/// Checks that an empty dispatch and words at a misaligned offset are refused, which Metal's validation stops.
pub fn refuses_empty_dispatches_and_misaligned_words<M: Metal>(mut subject: M) {
    let words = filled(&mut subject, &[0; (WORDS_BYTES * 2) as usize]);
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
        copy(0, RGBA8_BYTES * 2, 3),
        fill(u64::MIN, u64::MIN),
        fill(1, RGBA8_BYTES),
        fill(0, 3),
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
    refuses_blit(&mut subject, copy(BEYOND_ROW, 0, 1));
    // A 3D texture's rows are as long as its own longest side at most, shorter than a 2D texture's (MEASURED: Metal's
    // validation stops 24000 bytes past 16384, found by the fuzzer).
    let volume = made(&mut subject, described(TextureType::Type3D, PixelFormat::Rgba8Unorm));
    refuses_blit(
        &mut subject,
        BlitCommand::CopyFromBufferToTexture {
            source_buffer: buffer.clone(),
            source_offset: u64::MIN,
            source_bytes_per_row: u64::from(BEYOND_3D_SIDE) * RGBA8_BYTES,
            source_bytes_per_image: 0,
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
    let color = filled(&mut subject, &[0; COLOR_BYTES as usize]);
    let texture = target(&mut subject);
    let results = filled(&mut subject, &[0; VISIBILITY_RESULT_BYTES as usize]);
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
    let mut read = [0; VISIBILITY_RESULT_BYTES as usize];
    subject
        .read_contents(&results, 0, &mut read)
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
        assert!(subject.write_contents(&buffer, offset, &[0; 8]).is_err());
        assert!(subject.read_contents(&buffer, offset, &mut [0; 8]).is_err());
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
    let results = filled(&mut subject, &[0; VISIBILITY_RESULT_BYTES as usize]);
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
            vertex_count: TRIANGLE_VERTICES,
            instance_count: SINGLE,
            base_instance: u32::MIN,
        },
        RenderCommand::DrawPrimitives {
            primitive_type: PrimitiveType::Triangle,
            vertex_start: u32::MIN,
            vertex_count: TRIANGLE_VERTICES,
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
            index_count: TRIANGLE_VERTICES,
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
    let mut read = vec![0; bytes.len()];
    subject
        .read_contents(&buffer, 0, &mut read)
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
    let color = filled(&mut subject, &[0; COLOR_BYTES as usize]);
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
