//! A compiler giving numbers outside the bridge's enumerations: every one is refused by name
//! (`ConvertError::Unknown`), never read as a variant it is not. The compiler is a stand-in
//! implementation of the Mesa compiler trait, so no Mesa is reached.
#![cfg(all(mesa, feature = "standalone"))]

use std::cell::RefCell;
use wcontainers_mesa_compiler::{
    Binding, Flat, FlatGeometry, FlatTessellation, FlatType, GlslName, GlslProgram, MesaCompiler,
    NearestSampler, PositionTransform, SlotRange, StreamOutput, TextureSwizzle,
};
use wcontainers_nir::ConvertError;
use wcontainers_nir::mesa::{BIND_RESULT, GLSL_NAME_KIND, Nir, SLOT_KIND, Stage};
use wcontainers_shaders::ENTRY_POINT;

/// What the stand-in gives back.
#[derive(Clone, Default)]
struct Given {
    flat: Option<Flat>,
    common_form: Option<Result<Vec<Binding>, u32>>,
    glsl_names: Vec<GlslName>,
}

thread_local! {
    static GIVEN: RefCell<Given> = RefCell::new(Given::default());
}

/// A compiler whose answers the test sets.
#[derive(Clone)]
struct Hostile;

impl MesaCompiler for Hostile {
    type Shader = ();

    fn new() -> Hostile {
        Hostile
    }
    fn flatten(&self, _: &mut (), _: u32) -> Option<Flat> {
        GIVEN.with(|g| g.borrow().flat.clone())
    }
    fn spirv_to_nir(&self, _: &[u32], _: u32, _: &core::ffi::CStr) -> Option<()> {
        Some(())
    }
    fn glsl_to_nir(&self, _: [Option<&core::ffi::CStr>; 3]) -> Result<GlslProgram<()>, Vec<u8>> {
        let names = GIVEN.with(|g| g.borrow().glsl_names.clone());
        Ok(GlslProgram {
            stages: [Some(()), None, None],
            names: [names, Vec::new(), Vec::new()],
            log: None,
        })
    }
    fn to_common_form(&self, _: &mut (), _: u32, _: &[SlotRange; 4]) -> Result<Vec<Binding>, u32> {
        GIVEN
            .with(|g| g.borrow().common_form.clone())
            .unwrap_or(Ok(Vec::new()))
    }
    fn lower_io(&self, _: &mut ()) {}
    fn alpha_to_one(&self, _: &mut ()) {}
    fn serialize(&self, _: &()) -> Option<Vec<u8>> {
        Some(Vec::new())
    }
    fn to_spirv(&self, _: &mut ()) -> Option<Vec<u32>> {
        None
    }
    fn cloned(&self, _: &()) -> Option<()> {
        Some(())
    }
    fn number_vertex_elements(&self, _: &mut ()) -> u64 {
        0
    }
    fn geometry_vertices_out(&self, _: &()) -> u32 {
        0
    }
    fn named_layer_location(&self, _: &()) -> u32 {
        u32::MAX
    }
    fn control_points_out(&self, _: &()) -> u32 {
        0
    }
    fn makes_points(&self, _: &()) -> bool {
        false
    }
    fn fragcolor_to_buffers(&self, _: &mut (), _: u32) {}
    fn split_resource_arrays(&self, _: &mut ()) {}
    fn textures_1d_as_2d(&self, _: &mut ()) {}
    fn drop_name(&self, _: &mut ()) {}
    fn uniform_blocks_as_words(&self, _: &mut ()) {}
    fn transform_position(&self, _: &mut (), _: PositionTransform) {}
    fn reads_sample_positions(&self, _: &mut ()) -> bool {
        false
    }
    fn interpolate_elsewhere(&self, _: &mut (), _: u32) -> bool {
        true
    }
    fn fit_sample_masks(&self, _: &mut (), _: bool) {}
    fn writes_sample_mask(&self, _: &mut ()) -> bool {
        false
    }
    fn fetches_target(&self, _: &mut ()) -> bool {
        false
    }
    fn formatless_images(&self, _: &mut ()) -> u64 {
        0
    }
    fn fetch_from_texture(&self, _: &mut (), _: u32, _: bool) {}
    fn swizzle_textures(&self, _: &mut (), _: &[TextureSwizzle]) {}
    fn nearest_integer_textures(&self, _: &mut (), _: &[NearestSampler]) -> bool {
        true
    }
    fn integer_sampled_units(&self, _: &mut ()) -> u64 {
        0
    }
    fn stream_point_size_at_a_location(&self, _: &mut ()) {}
    fn generic_end(&self, _: &()) -> u32 {
        0
    }
    fn point_size_also_at(&self, _: &mut (), _: u32) -> bool {
        false
    }
    fn point_size_at_most(&self, _: &(), _: f32) -> bool {
        false
    }
    fn point_square(&self, _: &(), _: u32, _: f32) -> Option<()> {
        None
    }
    fn geometry_squares(&self, _: &(), _: u32, _: f32, _: u32) -> Option<()> {
        None
    }
    fn storage_pass_through(&self, _: &mut ()) -> Option<[Option<()>; 5]> {
        None
    }
    fn stream_outputs(&self, _: &(), _: &mut [StreamOutput], _: &mut [u32]) -> u32 {
        0
    }
}

/// A compute program with nothing in it, which route B takes.
fn empty() -> Flat {
    Flat {
        stage: wcontainers_mesa_compiler::constants::FLAT_COMPUTE,
        workgroup_size: [1, 1, 1],
        early_fragment_tests: 0,
        values: 0,
        types: Vec::new(),
        fields: Vec::new(),
        vars: Vec::new(),
        initialisers: Vec::new(),
        instrs: Vec::new(),
        geometry: FlatGeometry {
            input: 0,
            output: 0,
            vertices_in: 0,
            max_vertices: 0,
            invocations: 0,
            streams: 0,
        },
        tessellation: FlatTessellation {
            points_in: 0,
            points_out: 0,
            domain: 0,
            spacing: 0,
            counter_clockwise: 0,
            point_mode: 0,
        },
    }
}

/// Route B's answer for the flat form `flat`.
fn routed(flat: Flat) -> Result<(), ConvertError> {
    GIVEN.with(|g| g.borrow_mut().flat = Some(flat));
    let mut nir = Nir::<Hostile>::from_spirv(&[], Stage::Compute, ENTRY_POINT)
        .expect("the stand-in makes one");
    wcontainers_nir::direct::route(&mut nir).map(|_| ())
}

/// Far past any enumeration's last value.
const BEYOND: u32 = 0x8000_0001;

#[test]
fn the_empty_program_is_taken() {
    assert!(routed(empty()).is_ok());
}

#[test]
fn a_stage_outside_its_enumeration_is_refused_by_name() {
    let mut flat = empty();
    flat.stage = BEYOND;
    assert_eq!(
        routed(flat),
        Err(ConvertError::Unknown("enum wc_stage_kind", BEYOND))
    );
}

#[test]
fn a_type_kind_outside_its_enumeration_is_refused_by_name() {
    let mut flat = empty();
    flat.types.push(FlatType {
        kind: BEYOND,
        scalar: 0,
        bits: 32,
        components: 1,
        columns: 1,
        length: 0,
        element: 0,
        stride: 0,
        row_major: 0,
        dim: 0,
        arrayed: 0,
        shadow: 0,
        multisampled: 0,
        sampled: 0,
    });
    assert_eq!(
        routed(flat),
        Err(ConvertError::Unknown("enum wc_type_kind", BEYOND))
    );
}

#[test]
fn a_geometry_input_outside_its_enumeration_is_refused_by_name() {
    let mut flat = empty();
    flat.geometry.input = BEYOND;
    assert_eq!(
        routed(flat),
        Err(ConvertError::Unknown("enum wc_geometry_input", BEYOND))
    );
}

#[test]
fn a_binding_result_outside_its_enumeration_is_refused_by_name() {
    GIVEN.with(|g| g.borrow_mut().common_form = Some(Err(BEYOND)));
    let mut nir = Nir::<Hostile>::from_spirv(&[], Stage::Compute, ENTRY_POINT)
        .expect("the stand-in makes one");
    assert_eq!(
        nir.to_common_form(Stage::Compute),
        Err(ConvertError::Unknown(BIND_RESULT, BEYOND))
    );
}

#[test]
fn a_binding_kind_outside_its_enumeration_is_refused_by_name() {
    let binding = Binding {
        kind: BEYOND,
        set: 0,
        binding: 0,
        count: 1,
        new_binding: 0,
        stride: 0,
    };
    GIVEN.with(|g| g.borrow_mut().common_form = Some(Ok(vec![binding])));
    let mut nir = Nir::<Hostile>::from_spirv(&[], Stage::Compute, ENTRY_POINT)
        .expect("the stand-in makes one");
    assert_eq!(
        nir.to_common_form(Stage::Compute),
        Err(ConvertError::Unknown(SLOT_KIND, BEYOND))
    );
}

#[test]
fn a_glsl_name_kind_outside_its_enumeration_is_refused_by_name() {
    let name = GlslName {
        kind: BEYOND,
        name: Some(b"x".to_vec()),
        type_: None,
        interpolation: 0,
        at: 0,
        count: 1,
        element: 0,
    };
    GIVEN.with(|g| g.borrow_mut().glsl_names = vec![name]);
    let refused = Nir::<Hostile>::from_glsl(&[(Stage::Compute, "void main() {}")]).map(|_| ());
    assert_eq!(refused, Err(ConvertError::Unknown(GLSL_NAME_KIND, BEYOND)));
}
