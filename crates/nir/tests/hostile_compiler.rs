//! A compiler giving numbers outside the bridge's enumerations: every one is refused by name
//! (`ConvertError::Unknown`), never read as a variant it is not. The compiler is a stand-in
//! implementation of the Mesa compiler trait, so no Mesa is reached.
#![cfg(all(mesa, feature = "standalone"))]

use std::cell::RefCell;
use wcontainers_mesa_compiler::{
    Binding, Flat, FlatGeometry, FlatTessellation, FlatType, GlslName, GlslProgram, MesaCompiler,
    NearestSampler, PreparedForm, SlotRange, SpirvCommonForm, Squares, TextureSwizzle, VariantForm,
    VariantSettings, constants,
};
use wcontainers_nir::ConvertError;
use wcontainers_nir::mesa::{
    BIND_RESULT, DrawSettings, GLSL_NAME_KIND, GlslStage, SLOT_KIND, SQUARES_KIND, SpirvProgram,
    Stage, VARIANT_RESULT,
};
use wcontainers_shaders::ENTRY_POINT;

/// What the stand-in gives back.
#[derive(Clone)]
struct Given {
    flat: Option<Flat>,
    common_form_result: u32,
    bindings: Vec<Binding>,
    glsl_names: Vec<GlslName>,
    variant_result: u32,
    squares_kind: u32,
}

impl Default for Given {
    /// Every step made, nothing in it.
    fn default() -> Given {
        Given {
            flat: None,
            common_form_result: constants::BIND_OK,
            bindings: Vec::new(),
            glsl_names: Vec::new(),
            variant_result: constants::VARIANT_MADE,
            squares_kind: constants::SQUARES_ONE_PIXEL,
        }
    }
}

thread_local! {
    static GIVEN: RefCell<Given> = RefCell::new(Given::default());
}

/// What the test set.
fn given() -> Given {
    GIVEN.with(|g| g.borrow().clone())
}

/// A compiler whose answers the test sets.
#[derive(Clone)]
struct Hostile;

impl MesaCompiler for Hostile {
    type SpirvShader = ();
    type SpirvCommon = ();
    type Received = ();
    type Prepared = ();
    type Made = ();
    type Variant = ();

    fn new() -> Hostile {
        Hostile
    }
    fn spirv_read(&self, _: &[u32], _: u32, _: &core::ffi::CStr, _: bool) -> Option<()> {
        Some(())
    }
    fn spirv_shader_key(&self, _: &()) -> Option<Vec<u8>> {
        Some(Vec::new())
    }
    fn spirv_to_common_form(&self, _: (), _: u32, _: &[SlotRange; 4]) -> SpirvCommonForm<()> {
        let given = given();
        SpirvCommonForm {
            shader: Some(()),
            bindings: given.bindings,
            result: given.common_form_result,
        }
    }
    fn spirv_common_to_spirv(&self, _: ()) -> Option<Vec<u32>> {
        None
    }
    fn spirv_common_flatten(&self, _: ()) -> Option<Flat> {
        given().flat
    }
    fn glsl_to_nir(&self, _: [Option<&core::ffi::CStr>; 3]) -> Result<GlslProgram<()>, Vec<u8>> {
        Ok(GlslProgram {
            stages: [Some(()), None, None],
            names: [given().glsl_names, Vec::new(), Vec::new()],
            log: None,
        })
    }
    fn received_prepare(&self, _: ()) -> PreparedForm<(), ()> {
        PreparedForm {
            shader: Some(()),
            key: Vec::new(),
            streamed: Vec::new(),
            strides: [0; 4],
            streams_elsewhere: false,
            reads_sample_positions: false,
            writes_sample_mask: false,
            fetches_target: false,
            formatless_images: 0,
            integer_sampled_units: 0,
            named_layer: u32::MAX,
            passes_through: false,
            pass_through: [None, None, None, None, None],
            generic_end: 0,
            geometry_vertices_out: 0,
            control_points_out: 0,
            makes_points: false,
        }
    }
    fn prepared_variant(
        &self,
        _: &(),
        _: VariantSettings,
        _: &[TextureSwizzle],
        _: &[NearestSampler],
    ) -> VariantForm<()> {
        VariantForm {
            shader: Some(()),
            result: given().variant_result,
            key: Vec::new(),
            bindings: Vec::new(),
        }
    }
    fn prepared_points_as_squares(&self, _: &(), _: u32, _: f32) -> Squares<()> {
        Squares {
            kind: given().squares_kind,
            sized: false,
            before: None,
            geometry: None,
        }
    }
    fn prepared_geometry_points_as_squares(
        &self,
        vs: &(),
        at: u32,
        largest: f32,
        _: u32,
    ) -> Squares<()> {
        self.prepared_points_as_squares(vs, at, largest)
    }
    fn made_key(&self, _: &()) -> Option<Vec<u8>> {
        Some(Vec::new())
    }
    fn made_named_layer_location(&self, _: &()) -> u32 {
        u32::MAX
    }
    fn made_variant(
        &self,
        shader: &(),
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<()> {
        self.prepared_variant(shader, settings, swizzles, samplers)
    }
    fn variant_flatten(&self, _: (), _: u32) -> Option<Flat> {
        given().flat
    }
}

/// A program the stand-in read, in the common form with its bindings checked.
fn common_form() -> Result<(), ConvertError> {
    let alpha_to_one = false;
    SpirvProgram::<Hostile>::read(&[], Stage::Compute, ENTRY_POINT, alpha_to_one)
        .expect("the stand-in makes one")
        .to_common_form(Stage::Compute)
        .map(|_| ())
}

/// The stand-in's GLSL program, prepared.
fn prepared() -> wcontainers_nir::mesa::Prepared<Hostile> {
    GlslStage::<Hostile>::compile(&[(Stage::Compute, "void main() {}")])
        .expect("the stand-in compiles it")
        .pop()
        .expect("one stage")
        .received
        .prepare()
        .expect("the stand-in prepares it")
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
    wcontainers_nir::direct::route(flat).map(|_| ())
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
    GIVEN.with(|g| g.borrow_mut().common_form_result = BEYOND);
    assert_eq!(
        common_form(),
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
    GIVEN.with(|g| g.borrow_mut().bindings = vec![binding]);
    assert_eq!(common_form(), Err(ConvertError::Unknown(SLOT_KIND, BEYOND)));
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
    let refused = GlslStage::<Hostile>::compile(&[(Stage::Compute, "void main() {}")]).map(|_| ());
    assert_eq!(refused, Err(ConvertError::Unknown(GLSL_NAME_KIND, BEYOND)));
}

#[test]
fn a_variant_result_outside_its_enumeration_is_refused_by_name() {
    GIVEN.with(|g| g.borrow_mut().variant_result = BEYOND);
    let refused = prepared()
        .variant(Stage::Compute, &DrawSettings::default(), &[], &[])
        .map(|_| ());
    assert_eq!(refused, Err(ConvertError::Unknown(VARIANT_RESULT, BEYOND)));
}

#[test]
fn a_squares_kind_outside_its_enumeration_is_refused_by_name() {
    GIVEN.with(|g| g.borrow_mut().squares_kind = BEYOND);
    let refused = prepared().points_as_squares(0, 1.0).map(|_| ());
    assert_eq!(refused, Err(ConvertError::Unknown(SQUARES_KIND, BEYOND)));
}

#[test]
fn squares_made_without_their_program_are_refused_by_name() {
    GIVEN.with(|g| g.borrow_mut().squares_kind = constants::SQUARES_MADE);
    let refused = prepared().points_as_squares(0, 1.0).map(|_| ());
    assert_eq!(
        refused,
        Err(ConvertError::Unknown(SQUARES_KIND, constants::SQUARES_MADE))
    );
}
