//! How `depth24plus` is stored (plan section 4.11): a triangle at depth 0.1 drawn with a constant
//! bias of one unit into a 1 by 1 target, its depth loaded into an `r32float` target and read back.
//! One 24-bit step above 0.1 is 24-bit unorm storage (`DEPTH24_UNORM`); one float bit above (2^-27)
//! is float storage.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The depth the probe's triangle is drawn at.
const PROBE_DEPTH: f32 = 0.1;

/// How far a read may be from the biased 24-bit step, in steps: one, with room for the read's
/// own rounding.
const STEP_TOLERANCE: f64 = 1.001;

/// The triangle at `PROBE_DEPTH`, and the depth loaded as a colour.
fn probe_wgsl() -> String {
    format!(
        "
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {{
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, {PROBE_DEPTH:?}, 1.0);
}}
@group(0) @binding(0) var depth: texture_2d<f32>;
@fragment fn load(@builtin(position) p: vec4f) -> @location(0) vec4f {{
    return vec4f(textureLoad(depth, vec2i(p.xy), 0).x, 0.0, 0.0, 1.0);
}}
"
    )
}

/// What the probe found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Depth24 {
    /// 24-bit unsigned normalised: the biased depth read back.
    Unorm(f32),
    /// 32-bit float.
    Float(f32),
    /// Neither: the bias moves depth by another amount (wgpu's Vulkan on the RTX 3080 moves it by
    /// two 24-bit steps), so fronts bias in the program there.
    Other(f32),
}

impl Depth24 {
    /// Reads the answer: the depth 0.1 lands on, biased by one unit.
    pub fn of(read: f32) -> Depth24 {
        let base = PROBE_DEPTH;
        let max = f64::from(crate::fill::UNORM24_MAX);
        // The 24-bit step 0.1 is stored as, one above; read back within one step, as programs read
        // 24-bit depth.
        let step = (f64::from(base) * max).round() + 1.0;
        if (f64::from(read) * max - step).abs() < STEP_TOLERANCE {
            Depth24::Unorm(read)
        } else if read.to_bits() == base.to_bits() + 1 {
            Depth24::Float(read)
        } else {
            Depth24::Other(read)
        }
    }
}

/// Draws the probe and waits for its answer.
pub fn run(device: &wgpu::Device, queue: &wgpu::Queue) -> Depth24 {
    let texture = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let depth = texture(
        wgpu::TextureFormat::Depth24Plus,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let color = texture(
        wgpu::TextureFormat::R32Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(probe_wgsl().into()),
    });
    let vertex = wgpu::VertexState {
        module: &module,
        entry_point: None,
        compilation_options: Default::default(),
        buffers: &[],
    };
    let biased = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: vertex.clone(),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: wgpu::DepthBiasState {
                constant: 1,
                slope_scale: 0.0,
                clamp: 0.0,
            },
        }),
        multisample: Default::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    });
    let group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&group_layout)],
        immediate_size: 0,
    });
    let load = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex,
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::R32Float.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let depth_view = depth.create_view(&Default::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&depth_view),
        }],
    });
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: size_of::<f32>() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&biased);
        rp.draw(crate::fill::COVER_VERTICES, 0..1);
    }
    {
        let view = color.create_view(&Default::default());
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&load);
        rp.set_bind_group(0, &group, &[]);
        rp.draw(crate::fill::COVER_VERTICES, 0..1);
    }
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let index = queue.submit([encoder.finish()]);
    let mapped = Arc::new(AtomicBool::new(false));
    let flag = mapped.clone();
    read.slice(..).map_async(wgpu::MapMode::Read, move |_| {
        flag.store(true, Ordering::Release);
        crate::wait::wake();
    });
    crate::wait::until(device, Some(index), || mapped.load(Ordering::Acquire));
    let value = match read.slice(..).get_mapped_range() {
        Ok(bytes) => f32::from_le_bytes(*bytes.first_chunk().expect("one depth value")),
        Err(_) => f32::NAN,
    };
    Depth24::of(value)
}
