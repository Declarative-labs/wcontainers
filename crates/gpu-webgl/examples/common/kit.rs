//! Small drawing helpers the back end's checks share, generic over the back end so a check can run on
//! WebGL 2 and on WebGPU alike.

use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, helpers::*, occlusion::*, passes::*, reflect::*,
    resource::*, shader::*, slow::*, state::*, *,
};

/// The blocks most checks draw with: opaque writes of every channel, no depth or stencil test, no
/// culling, no vertex streams.
pub struct Kit<B: Gpu> {
    pub blend: B::Blend,
    pub no_depth: B::DepthStencil,
    pub raster: B::Raster,
    pub layout: B::VertexLayout,
}

impl<B: Gpu> Kit<B> {
    pub fn new(b: &mut B) -> Kit<B> {
        Kit {
            blend: blend(b, None),
            no_depth: depth_stencil(b, None, false, None),
            raster: raster(b, CullMode::None, FrontFace::Clockwise, 0.0, 0.0),
            layout: b
                .create_vertex_layout(&VertexLayoutDesc {
                    streams: [None; MAX_VERTEX_STREAMS],
                    attributes: [None; MAX_VERTEX_STREAMS],
                })
                .expect("create_vertex_layout"),
        }
    }
}

pub fn program<B: Gpu>(b: &mut B, wgsl: &str, vs: &str, fs: &str) -> B::Program {
    let v = ShaderModule::from_wgsl(wgsl, vs, ShaderStage::Vertex).expect("the WGSL validates");
    let f = ShaderModule::from_wgsl(wgsl, fs, ShaderStage::Fragment).expect("the WGSL validates");
    let v = b.create_shader(&v).expect("create_shader");
    let f = b.create_shader(&f).expect("create_shader");
    b.create_program(&Program::Render {
        vertex: &v,
        fragment: Some(&f),
    })
    .expect("create_program")
}

/// A blend block applying `blend` (colour, alpha) to every target, or none.
pub fn blend<B: Gpu>(b: &mut B, blend: Option<(BlendComponent, BlendComponent)>) -> B::Blend {
    b.create_blend(&BlendDesc {
        targets: [RenderTargetBlend {
            blend,
            writes: ColorWrites::all(),
        }; MAX_COLOR_TARGETS],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    })
    .expect("create_blend")
}

pub fn depth_stencil<B: Gpu>(
    b: &mut B,
    depth: Option<CompareFn>,
    write: bool,
    stencil: Option<StencilFace>,
) -> B::DepthStencil {
    b.create_depth_stencil(&DepthStencilDesc {
        depth_compare: depth,
        depth_write: write,
        stencil: stencil.map(|s| (s, s)),
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    })
    .expect("create_depth_stencil")
}

/// A raster block in the clip depth convention the back end has, with a constant and a slope depth
/// bias.
pub fn raster<B: Gpu>(
    b: &mut B,
    cull: CullMode,
    front: FrontFace,
    bias: f32,
    slope: f32,
) -> B::Raster {
    let provoking = if b.caps().features.contains(Features::PROVOKING_VERTEX_FIRST) {
        ProvokingVertex::First
    } else {
        ProvokingVertex::Last
    };
    b.create_raster(&RasterDesc {
        fill: FillMode::Solid,
        cull,
        front,
        depth_bias: bias.to_bits(),
        depth_bias_slope: slope.to_bits(),
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking,
        depth_minus_one: !b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE),
        sample_mask: u32::MAX,
    })
    .expect("create_raster")
}

/// A texture of one sample.
pub fn texture<B: Gpu>(
    b: &mut B,
    dimension: TextureDimension,
    format: Format,
    size: [u32; 3],
    mips: u32,
    usage: TextureUsage,
    cube: bool,
) -> B::Texture {
    b.create_texture(&TextureDesc {
        dimension,
        format,
        width: size[0],
        height: size[1],
        depth_or_layers: size[2],
        mip_levels: mips,
        samples: 1,
        usage,
        cube,
        reinterpret: false,
    })
    .expect("create_texture")
}

/// A view of `mips` levels from `mip` and `layers` layers from `layer`.
#[allow(clippy::too_many_arguments)]
pub fn view<B: Gpu>(
    b: &mut B,
    texture: &B::Texture,
    format: Format,
    dimension: ViewDimension,
    mip: u32,
    mips: u32,
    layer: u32,
    layers: u32,
) -> B::TextureView {
    b.create_view(
        texture,
        &TextureViewDesc {
            format,
            dimension,
            base_mip: mip,
            mip_count: mips,
            base_layer: layer,
            layer_count: layers,
            stencil: false,
        },
    )
    .expect("create_view")
}

/// A view to draw into: one level and one layer.
pub fn target<B: Gpu>(
    b: &mut B,
    texture: &B::Texture,
    format: Format,
    mip: u32,
    layer: u32,
) -> B::TextureView {
    view(b, texture, format, ViewDimension::D2, mip, 1, layer, 1)
}

pub fn viewport(x: f32, y: f32, width: f32, height: f32) -> Viewport {
    Viewport {
        x: x.to_bits(),
        y: y.to_bits(),
        width: width.to_bits(),
        height: height.to_bits(),
        min_depth: 0f32.to_bits(),
        max_depth: 1f32.to_bits(),
    }
}

/// A pass on one colour target, cleared to `clear` (raw bits), with an optional depth-stencil one
/// cleared to `depth` and stencil 0.
pub fn pass<'a, B: Gpu>(
    colour: &'a B::TextureView,
    clear: [u32; 4],
    depth: Option<(&'a B::TextureView, f32, bool)>,
) -> PassDesc<'a, B> {
    PassDesc {
        colors: core::array::from_fn(|i| {
            (i == 0).then_some(ColorAttachment {
                view: colour,
                depth_slice: 0,
                load: LoadOp::Clear(clear),
                store: StoreOp::Store,
                resolve: None,
            })
        }),
        depth: depth.map(|(view, d, stencil)| DepthAttachment {
            view,
            depth: Some(AspectOps {
                load: LoadOp::Clear(d.to_bits()),
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: stencil.then_some(AspectOps {
                load: LoadOp::Clear(0),
                store: StoreOp::Store,
                read_only: false,
            }),
        }),
        occlusion: None,
        empty: None,
    }
}

/// A draw of `count` vertices with no streams, in `kit`'s blocks unless the caller changes them.
pub fn call<'a, B: Gpu>(
    kit: &'a Kit<B>,
    program: &'a B::Program,
    set: Option<&'a B::BindingSet>,
    viewports: &'a [Viewport],
    topology: Topology,
    count: u32,
) -> DrawCall<'a, B> {
    DrawCall {
        program,
        blend: &kit.blend,
        depth_stencil: &kit.no_depth,
        raster: &kit.raster,
        vertex_layout: &kit.layout,
        bindings: [set, None, None, None],
        vertex_buffers: &[],
        index: None,
        topology,
        restart: false,
        range: DrawRange::Vertices {
            first: 0,
            count,
            first_instance: 0,
            instances: 1,
        },
        stencil_reference: [0; 2],
        blend_constant: [0; 4],
        immediates: &[],
        viewports,
        scissors: &[],
    }
}

/// Reads `layers` layers of a `width` by `height` region of level `mip` from layer `layer`, now.
#[allow(clippy::too_many_arguments)]
pub fn read<B: Gpu>(
    b: &mut B,
    texture: &B::Texture,
    mip: u32,
    layer: u32,
    size: [u32; 3],
    texel: usize,
    stencil: bool,
) -> Vec<u8> {
    let mut got = vec![0u8; (size[0] * size[1] * size[2]) as usize * texel];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture,
                mip,
                layer,
                origin: Origin::default(),
                stencil,
            },
            extent: Extent {
                width: size[0],
                height: size[1],
                depth_or_layers: size[2],
            },
        },
        &mut got,
    )
    .expect("read_back_now");
    got
}
