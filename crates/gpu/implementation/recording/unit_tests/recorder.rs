//! The shared suite on the recording back end, and the interface rules it enforces.

use crate::Recorder;
use wcontainers_gpu::{caps::*, draw::*, error::*, format::*, resource::*, state::*, *};

/// What is drawn into the surface's current texture is what the recorder shows, frame after frame.
#[test]
fn present_shows_the_frame() {
    const WINDOW: u32 = 7;
    let mut b = Recorder::new();
    let surface = b.create_surface(WINDOW).expect("create_surface");
    b.configure_surface(&surface, Format::Rgba8Unorm, 4, 4, true)
        .expect("configure_surface");
    for (value, texel) in [
        ([0.2f32, 0.4, 0.6, 1.0], [51u8, 102, 153, 255]),
        ([1.0, 0.0, 0.2, 1.0], [255, 0, 51, 255]),
    ] {
        let v = b.current_texture(&surface).expect("current_texture");
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &v,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear(value.map(f32::to_bits)),
            store: StoreOp::Store,
        });
        b.begin_pass(&pass);
        b.end_pass();
        b.present(&surface).expect("present");
        let shown = b
            .shown
            .iter()
            .rev()
            .find(|f| f.window == WINDOW)
            .map(|f| f.bytes.clone())
            .unwrap_or_default();
        assert_eq!(shown, texel.repeat(16));
    }
}

fn target(r: &mut Recorder, width: u32, height: u32, format: Format) -> crate::Id {
    let t = r
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width,
            height,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    r.create_view(
        &t,
        &TextureViewDesc {
            format,
            dimension: ViewDimension::D2,
            base_mip: 0,
            mip_count: 1,
            base_layer: 0,
            layer_count: 1,
            stencil: false,
        },
    )
    .unwrap()
}

fn pass_on<'a>(color: &'a crate::Id, depth: Option<&'a crate::Id>) -> PassDesc<'a, Recorder> {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: depth.map(|view| DepthAttachment {
            view,
            depth: Some(AspectOps {
                load: LoadOp::Load,
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: None,
        }),
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: color,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Load,
        store: StoreOp::Store,
    });
    pass
}

#[test]
fn attachments_of_different_sizes_are_refused() {
    let mut r = Recorder::new();
    let color = target(&mut r, 4, 4, Format::Rgba8Unorm);
    let depth = target(&mut r, 8, 8, Format::Depth32Float);
    r.begin_pass(&pass_on(&color, Some(&depth)));
    assert_eq!(r.violations, ["attachments of different sizes"]);
    assert_eq!(r.status(), DeviceStatus::Lost(LossReason::Error));
}

#[test]
fn depth_aspects_must_match_the_format() {
    let mut r = Recorder::new();
    let color = target(&mut r, 4, 4, Format::Rgba8Unorm);
    let depth = target(&mut r, 4, 4, Format::Depth24PlusStencil8);
    r.begin_pass(&pass_on(&color, Some(&depth)));
    assert_eq!(
        r.violations,
        ["depth attachment aspects differ from the format's"]
    );
}

#[test]
fn copies_and_read_backs_inside_a_pass_are_refused() {
    let mut r = Recorder::new();
    let color = target(&mut r, 4, 4, Format::Rgba8Unorm);
    let buffer = r
        .create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    r.begin_pass(&pass_on(&color, None));
    r.copy_buffer(&buffer, 0, &buffer, 8, 8);
    assert!(
        r.read_back(&ReadSource::Buffer {
            buffer: &buffer,
            offset: 0,
            size: 4,
        })
        .is_err()
    );
    r.end_pass();
    assert_eq!(
        r.violations,
        ["copy_buffer inside a pass", "read_back inside a pass"]
    );
}

#[test]
fn a_finished_read_before_submit_is_refused() {
    let mut r = Recorder::new();
    let buffer = r
        .create_buffer(&BufferDesc {
            size: 4,
            usage: BufferUsage::COPY_SRC,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    let rb = r
        .read_back(&ReadSource::Buffer {
            buffer: &buffer,
            offset: 0,
            size: 4,
        })
        .unwrap();
    assert_eq!(r.read_state(&rb), ReadState::Pending);
    r.finish_read(rb, &mut [0; 4]);
    assert_eq!(
        r.violations,
        ["finish_read of a read back that is not ready"]
    );
}

#[test]
fn objects_used_after_destroy_are_refused() {
    let mut r = Recorder::new();
    let buffer = r
        .create_buffer(&BufferDesc {
            size: 4,
            usage: BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    r.destroy(Object::Buffer(buffer));
    r.write_buffer(&buffer, 0, &[1], WriteMode::Ordered);
    r.destroy(Object::Buffer(buffer));
    assert_eq!(r.violations.len(), 2);
}

#[test]
fn blocks_needing_absent_features_are_refused_at_creation() {
    let mut r = Recorder::new();
    let mut desc = BlendDesc {
        targets: [RenderTargetBlend {
            blend: None,
            writes: ColorWrites::all(),
        }; MAX_COLOR_TARGETS],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    };
    let second_source = BlendComponent {
        src: BlendFactor::One,
        dst: BlendFactor::OneMinusSrc1Alpha,
        op: BlendOp::Add,
    };
    desc.targets[0].blend = Some((second_source, second_source));
    assert_eq!(
        r.create_blend(&desc).unwrap_err(),
        GpuError::Absent(Features::DUAL_SOURCE_BLEND)
    );
    desc.targets[0].blend = None;
    desc.logic_op = Some(LogicOp::Xor);
    assert_eq!(
        r.create_blend(&desc).unwrap_err(),
        GpuError::Absent(Features::LOGIC_OP)
    );
    assert!(r.violations.is_empty());
    assert_eq!(r.status(), DeviceStatus::Ok);
}
