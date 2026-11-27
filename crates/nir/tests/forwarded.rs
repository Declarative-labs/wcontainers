//! What src/mesa.rs hands Mesa's compiler and what it gives back, stated exactly: a stand-in
//! compiler records every call with its arguments and answers with values the test chose, and each
//! test says the one right record and the one right result, from the documented behaviour of each
//! step (its doc comment and the bridge's enumerations), never by comparing two of our own results.
#![cfg(all(mesa, feature = "standalone"))]

use std::cell::RefCell;
use std::ffi::CStr;
use wcontainers_gpu::reflect::{ReflectedBinding, SlotKind};
use wcontainers_gpu::shader::ShaderStage;
use wcontainers_mesa_compiler::{
    Binding, Flat, FlatGeometry, FlatTessellation, GlslProgram, MesaCompiler, NearestSampler,
    PositionTransform, PreparedForm, SlotRange, SpirvCommonForm, Squares, StreamOutput,
    TextureSwizzle, VariantForm, VariantSettings, constants,
};
use wcontainers_nir::ConvertError;
use wcontainers_nir::bind::{BEYOND_LIMIT, OUT_OF_MEMORY, UNBOUNDED};
use wcontainers_nir::direct::FLATTEN;
use wcontainers_nir::mesa::{
    CONVERTER_VERSION, DrawSettings, GLSL_INTERPOLATION, GLSL_NAME_KIND, GlslName, GlslNameKind,
    Interpolation, NIR_SERIALIZE, SLOT_KIND, SPIRV_TO_NIR, SQUARES_KIND, Stage, StreamedOutput,
    Swizzle, VARIANT_RESULT, Wrap,
};

/// The stand-in's programs: a number naming each, so a record says which program a call was given.
type Tag = u32;

/// The programs `glsl_to_nir` gives, one a stage in the bridge's order.
const VERTEX: Tag = 10;
const FRAGMENT: Tag = 11;
const COMPUTE: Tag = 12;
const GEOMETRY: Tag = 13;
const TESS_CONTROL: Tag = 14;
const TESS_EVALUATION: Tag = 15;
/// What the stand-in's prepare steps give: the program of the received one, plus this.
const PREPARED: Tag = 100;
/// The program `spirv_read` gives and the common form made of it.
const READ: Tag = 1;
const COMMON: Tag = 2;
/// The variant the variant steps give.
const VARIANT: Tag = 40;

/// Every stage, its received program and the bridge's number for it (`enum wc_stage`).
const STAGES: [(Stage, Tag, u32); 6] = [
    (Stage::Vertex, VERTEX, constants::STAGE_VERTEX),
    (Stage::Fragment, FRAGMENT, constants::STAGE_FRAGMENT),
    (Stage::Compute, COMPUTE, constants::STAGE_COMPUTE),
    (Stage::Geometry, GEOMETRY, constants::STAGE_GEOMETRY),
    (
        Stage::TessControl,
        TESS_CONTROL,
        constants::STAGE_TESS_CONTROL,
    ),
    (
        Stage::TessEvaluation,
        TESS_EVALUATION,
        constants::STAGE_TESS_EVALUATION,
    ),
];

/// One call the stand-in received, with its arguments.
#[derive(Clone, Debug, PartialEq)]
enum Call {
    SpirvRead {
        words: Vec<u32>,
        stage: u32,
        entry: String,
        alpha_to_one: bool,
    },
    SpirvKey(Tag),
    CommonForm {
        shader: Tag,
        set: u32,
        ranges: [SlotRange; 4],
    },
    CommonFlatten(Tag),
    GlslToNir([Option<String>; 6]),
    Prepare {
        step: &'static str,
        shader: Tag,
    },
    Variant {
        step: &'static str,
        shader: Tag,
        settings: VariantSettings,
        swizzles: Vec<TextureSwizzle>,
        samplers: Vec<NearestSampler>,
    },
    Squares {
        step: &'static str,
        shader: Tag,
        at: u32,
        largest: f32,
        block: Option<u32>,
    },
    MadeKey(Tag),
    MadeNamedLayer(Tag),
    Flatten {
        shader: Tag,
        points_in: u32,
    },
}

/// What the stand-in answers, each taken by the next call of its step (its default otherwise).
#[derive(Default)]
struct Answers {
    read: Option<Option<Tag>>,
    key: Option<Option<Vec<u8>>>,
    common: Option<SpirvCommonForm<Tag>>,
    flat: Option<Option<Flat>>,
    glsl: Option<Result<GlslProgram<Tag, Tag, Tag, Tag, Tag, Tag>, Vec<u8>>>,
    prepared: Option<PreparedForm<Tag, Tag>>,
    variant: Option<VariantForm<Tag>>,
    squares: Option<Squares<Tag>>,
    named_layer: Option<u32>,
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static ANSWERS: RefCell<Answers> = RefCell::new(Answers::default());
}

/// Records `call`.
fn record(call: Call) {
    CALLS.with(|c| c.borrow_mut().push(call));
}

/// The calls recorded since the last time, which are forgotten.
fn calls() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// Sets what the stand-in answers next.
fn answer(set: impl FnOnce(&mut Answers)) {
    ANSWERS.with(|a| set(&mut a.borrow_mut()));
}

/// Takes one of the stand-in's answers.
fn take<T>(get: impl FnOnce(&mut Answers) -> Option<T>) -> Option<T> {
    ANSWERS.with(|a| get(&mut a.borrow_mut()))
}

/// A form with nothing in it, its program `shader`.
fn plain_form(shader: Tag) -> PreparedForm<Tag, Tag> {
    PreparedForm {
        shader: Some(shader),
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

/// A variant form made with nothing bound.
fn made_variant() -> VariantForm<Tag> {
    VariantForm {
        shader: Some(VARIANT),
        result: constants::VARIANT_MADE,
        key: Vec::new(),
        bindings: Vec::new(),
    }
}

/// The variant answer, else one made.
fn variant_answer() -> VariantForm<Tag> {
    take(|a| a.variant.take()).unwrap_or_else(made_variant)
}

/// The squares answer, else one pixel.
fn squares_answer() -> Squares<Tag> {
    take(|a| a.squares.take()).unwrap_or(Squares {
        kind: constants::SQUARES_ONE_PIXEL,
        sized: false,
        before: None,
        geometry: None,
    })
}

/// A compiler recording its calls.
#[derive(Clone)]
struct Recorder;

impl Recorder {
    /// Records a prepare step and answers its form, else a plain one of the program after it.
    fn prepare(step: &'static str, shader: Tag) -> PreparedForm<Tag, Tag> {
        record(Call::Prepare { step, shader });
        take(|a| a.prepared.take()).unwrap_or_else(|| plain_form(shader + PREPARED))
    }

    /// Records a variant step and answers its form.
    fn variant(
        step: &'static str,
        shader: Tag,
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<Tag> {
        record(Call::Variant {
            step,
            shader,
            settings,
            swizzles: swizzles.to_vec(),
            samplers: samplers.to_vec(),
        });
        variant_answer()
    }

    /// Records a squares step and answers its squares.
    fn squares(
        step: &'static str,
        shader: Tag,
        at: u32,
        largest: f32,
        block: Option<u32>,
    ) -> Squares<Tag> {
        record(Call::Squares {
            step,
            shader,
            at,
            largest,
            block,
        });
        squares_answer()
    }
}

impl MesaCompiler for Recorder {
    type SpirvShader = Tag;
    type SpirvCommon = Tag;
    type ReceivedVertex = Tag;
    type ReceivedFragment = Tag;
    type ReceivedCompute = Tag;
    type ReceivedGeometry = Tag;
    type ReceivedTessControl = Tag;
    type ReceivedTessEvaluation = Tag;
    type PreparedVertex = Tag;
    type PreparedFragment = Tag;
    type PreparedCompute = Tag;
    type PreparedGeometry = Tag;
    type PreparedTessControl = Tag;
    type PreparedTessEvaluation = Tag;
    type Made = Tag;
    type Variant = Tag;

    fn new() -> Recorder {
        Recorder
    }
    fn spirv_read(
        &self,
        words: &[u32],
        stage: u32,
        entry_point: &CStr,
        alpha_to_one: bool,
    ) -> Option<Tag> {
        record(Call::SpirvRead {
            words: words.to_vec(),
            stage,
            entry: entry_point.to_string_lossy().into_owned(),
            alpha_to_one,
        });
        take(|a| a.read.take()).unwrap_or(Some(READ))
    }
    fn spirv_shader_key(&self, shader: &Tag) -> Option<Vec<u8>> {
        record(Call::SpirvKey(*shader));
        take(|a| a.key.take()).unwrap_or(Some(Vec::new()))
    }
    fn spirv_to_common_form(
        &self,
        shader: Tag,
        set: u32,
        ranges: &[SlotRange; 4],
    ) -> SpirvCommonForm<Tag> {
        record(Call::CommonForm {
            shader,
            set,
            ranges: ranges.clone(),
        });
        take(|a| a.common.take()).unwrap_or(SpirvCommonForm {
            shader: Some(COMMON),
            bindings: Vec::new(),
            result: constants::BIND_OK,
        })
    }
    fn spirv_common_flatten(&self, shader: Tag) -> Option<Flat> {
        record(Call::CommonFlatten(shader));
        take(|a| a.flat.take()).unwrap_or(None)
    }
    fn glsl_to_nir(
        &self,
        sources: [Option<&CStr>; 6],
    ) -> Result<GlslProgram<Tag, Tag, Tag, Tag, Tag, Tag>, Vec<u8>> {
        record(Call::GlslToNir(
            sources.map(|s| s.map(|s| s.to_string_lossy().into_owned())),
        ));
        take(|a| a.glsl.take()).unwrap_or_else(|| Ok(every_stage()))
    }
    fn received_vertex_prepare(&self, shader: Tag) -> PreparedForm<Tag, Tag> {
        Recorder::prepare("vertex", shader)
    }
    fn received_fragment_prepare(&self, shader: Tag) -> PreparedForm<Tag, Tag> {
        Recorder::prepare("fragment", shader)
    }
    fn received_compute_prepare(&self, shader: Tag) -> PreparedForm<Tag, Tag> {
        Recorder::prepare("compute", shader)
    }
    fn received_geometry_prepare(&self, shader: Tag) -> PreparedForm<Tag, Tag> {
        Recorder::prepare("geometry", shader)
    }
    fn received_tess_control_prepare(&self, shader: Tag) -> PreparedForm<Tag, Tag> {
        Recorder::prepare("tess control", shader)
    }
    fn received_tess_evaluation_prepare(&self, shader: Tag) -> PreparedForm<Tag, Tag> {
        Recorder::prepare("tess evaluation", shader)
    }
    fn prepared_vertex_variant(
        &self,
        shader: &Tag,
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<Tag> {
        Recorder::variant("vertex", *shader, settings, swizzles, samplers)
    }
    fn prepared_fragment_variant(
        &self,
        shader: &Tag,
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<Tag> {
        Recorder::variant("fragment", *shader, settings, swizzles, samplers)
    }
    fn prepared_compute_variant(
        &self,
        shader: &Tag,
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<Tag> {
        Recorder::variant("compute", *shader, settings, swizzles, samplers)
    }
    fn prepared_geometry_variant(
        &self,
        shader: &Tag,
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<Tag> {
        Recorder::variant("geometry", *shader, settings, swizzles, samplers)
    }
    fn prepared_tess_control_variant(
        &self,
        shader: &Tag,
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<Tag> {
        Recorder::variant("tess control", *shader, settings, swizzles, samplers)
    }
    fn prepared_tess_evaluation_variant(
        &self,
        shader: &Tag,
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<Tag> {
        Recorder::variant("tess evaluation", *shader, settings, swizzles, samplers)
    }
    fn prepared_vertex_points_as_squares(
        &self,
        shader: &Tag,
        at: u32,
        largest: f32,
    ) -> Squares<Tag> {
        Recorder::squares("vertex", *shader, at, largest, None)
    }
    fn prepared_tess_evaluation_points_as_squares(
        &self,
        shader: &Tag,
        at: u32,
        largest: f32,
    ) -> Squares<Tag> {
        Recorder::squares("tess evaluation", *shader, at, largest, None)
    }
    fn prepared_geometry_points_as_squares(
        &self,
        shader: &Tag,
        at: u32,
        largest: f32,
        block: u32,
    ) -> Squares<Tag> {
        Recorder::squares("geometry", *shader, at, largest, Some(block))
    }
    fn made_key(&self, shader: &Tag) -> Option<Vec<u8>> {
        record(Call::MadeKey(*shader));
        take(|a| a.key.take()).unwrap_or(Some(Vec::new()))
    }
    fn made_named_layer_location(&self, shader: &Tag) -> u32 {
        record(Call::MadeNamedLayer(*shader));
        take(|a| a.named_layer.take()).unwrap_or(u32::MAX)
    }
    fn made_variant(
        &self,
        shader: &Tag,
        settings: VariantSettings,
        swizzles: &[TextureSwizzle],
        samplers: &[NearestSampler],
    ) -> VariantForm<Tag> {
        Recorder::variant("made", *shader, settings, swizzles, samplers)
    }
    fn variant_flatten(&self, shader: Tag, points_in: u32) -> Option<Flat> {
        record(Call::Flatten { shader, points_in });
        take(|a| a.flat.take()).unwrap_or(None)
    }
}

type SpirvProgram = wcontainers_nir::mesa::SpirvProgram<Recorder>;
type GlslStage = wcontainers_nir::mesa::GlslStage<Recorder>;
type Prepared = wcontainers_nir::mesa::Prepared<Recorder>;
type Made = wcontainers_nir::mesa::Made<Recorder>;

/// A program of every stage, with no names.
fn every_stage() -> GlslProgram<Tag, Tag, Tag, Tag, Tag, Tag> {
    GlslProgram {
        vertex: Some(VERTEX),
        fragment: Some(FRAGMENT),
        compute: Some(COMPUTE),
        geometry: Some(GEOMETRY),
        tess_control: Some(TESS_CONTROL),
        tess_evaluation: Some(TESS_EVALUATION),
        names: Default::default(),
        log: None,
    }
}

/// The cache's key of serialised bytes `bytes`, as `keyed`'s doc comment states it: BLAKE3 of the
/// bytes and then the converter's version.
fn key_of(bytes: &[u8]) -> [u8; 32] {
    let mut all = bytes.to_vec();
    all.extend_from_slice(CONVERTER_VERSION.as_bytes());
    *blake3::hash(&all).as_bytes()
}

/// The stage `stage` of the stand-in's program of every stage, received.
fn received(stage: Stage) -> GlslStage {
    let stages = GlslStage::compile(&[(stage, "void main() {}")]).unwrap();
    let found = stages.into_iter().find(|s| s.stage == stage).unwrap();
    calls();
    found
}

/// The stage `stage`, prepared with the stand-in's answer `form` (else a plain one).
fn prepared_with(stage: Stage, form: Option<PreparedForm<Tag, Tag>>) -> Prepared {
    let stage = received(stage);
    answer(|a| a.prepared = form);
    let prepared = stage
        .received
        .prepare()
        .unwrap_or_else(|_| panic!("prepared"));
    calls();
    prepared
}

/// The stage `stage`, prepared plainly.
fn prepared(stage: Stage) -> Prepared {
    prepared_with(stage, None)
}

/// A flat form, its workgroup `marker` wide, so two are told apart by what they are.
fn flat(marker: u32) -> Flat {
    let zero_geometry = FlatGeometry {
        input: 0,
        output: 0,
        vertices_in: 0,
        max_vertices: 0,
        invocations: 0,
        streams: 0,
    };
    let zero_tessellation = FlatTessellation {
        points_in: 0,
        points_out: 0,
        domain: 0,
        spacing: 0,
        counter_clockwise: 0,
        point_mode: 0,
    };
    Flat {
        stage: constants::FLAT_COMPUTE,
        workgroup_size: [marker, 1, 1],
        early_fragment_tests: 0,
        values: 0,
        types: Vec::new(),
        fields: Vec::new(),
        vars: Vec::new(),
        initialisers: Vec::new(),
        instrs: Vec::new(),
        geometry: zero_geometry,
        tessellation: zero_tessellation,
    }
}

/// Each binding kind's range as the gpu interface's `SLOT_KINDS` documents it, in the bridge's
/// order of kinds (uniform, sampler, texture, storage).
fn documented_ranges() -> [SlotRange; 4] {
    [
        SlotRange {
            first: 0,
            slots: 16,
        },
        SlotRange {
            first: 16,
            slots: 16,
        },
        SlotRange {
            first: 32,
            slots: 128,
        },
        SlotRange {
            first: 160,
            slots: 64,
        },
    ]
}

/// One binding of each kind the bridge numbers (`enum wc_slot_kind`), every field its own value.
fn one_binding_of_each_kind() -> Vec<Binding> {
    (0..4)
        .map(|kind| Binding {
            kind,
            set: 2 + kind,
            binding: 7 + kind,
            count: 3 + kind,
            new_binding: 40 + kind,
            stride: 16 * (kind + 1),
        })
        .collect()
}

/// What `one_binding_of_each_kind` reflects as in the set `set`.
fn reflected_in(set: u32) -> Vec<ReflectedBinding> {
    [
        SlotKind::Uniform,
        SlotKind::Sampler,
        SlotKind::Texture,
        SlotKind::Storage,
    ]
    .into_iter()
    .zip(0..)
    .map(|(kind, k)| ReflectedBinding {
        kind,
        space: 2 + k,
        register: 7 + k,
        count: 3 + k,
        set,
        binding: 40 + k,
        added: None,
        stride: 16 * (k + 1),
    })
    .collect()
}

/// The set each stage's own resources bind in, as `bind::stage_set` documents it.
fn documented_set(stage: Stage) -> u32 {
    match stage {
        Stage::Fragment => 1,
        _ => 0,
    }
}

#[test]
fn each_stage_binds_as_the_stage_its_doc_comment_names() {
    let documented = [
        (Stage::Vertex, ShaderStage::Vertex),
        (Stage::Fragment, ShaderStage::Fragment),
        (Stage::Compute, ShaderStage::Compute),
        (Stage::Geometry, ShaderStage::Compute),
        (Stage::TessControl, ShaderStage::Compute),
        (Stage::TessEvaluation, ShaderStage::Vertex),
    ];
    for (stage, shader_stage) in documented {
        assert_eq!(stage.shader_stage(), shader_stage, "{stage:?}");
    }
}

#[test]
fn read_hands_the_compiler_its_words_the_bridges_stage_number_its_entry_and_alpha_to_one() {
    for (stage, _, number) in STAGES {
        for alpha_to_one in [false, true] {
            assert!(SpirvProgram::read(&[3, 1, 4], stage, "main_entry", alpha_to_one).is_ok());
            assert_eq!(
                calls(),
                [Call::SpirvRead {
                    words: vec![3, 1, 4],
                    stage: number,
                    entry: "main_entry".into(),
                    alpha_to_one,
                }],
                "{stage:?}"
            );
        }
    }
}

#[test]
fn a_read_the_compiler_refuses_or_an_entry_with_a_nul_is_refused_at_spirv_read() {
    answer(|a| a.read = Some(None));
    let refused = SpirvProgram::read(&[], Stage::Vertex, "main", false).map(|_| ());
    assert_eq!(refused, Err(ConvertError::Mesa(SPIRV_TO_NIR)));
    calls();
    let refused = SpirvProgram::read(&[], Stage::Vertex, "ma\0in", false).map(|_| ());
    assert_eq!(refused, Err(ConvertError::Mesa(SPIRV_TO_NIR)));
    assert!(calls().is_empty(), "the compiler is not asked");
}

#[test]
fn a_read_programs_bytes_and_key_are_the_compilers_bytes_and_their_hash_with_the_version() {
    let program = SpirvProgram::read(&[], Stage::Compute, "main", false).unwrap();
    calls();
    answer(|a| a.key = Some(Some(vec![9, 8, 7])));
    assert_eq!(program.serialized(), Ok(vec![9, 8, 7]));
    assert_eq!(calls(), [Call::SpirvKey(READ)]);
    answer(|a| a.key = Some(Some(vec![9, 8, 7])));
    assert_eq!(program.key(), Ok(key_of(&[9, 8, 7])));
    answer(|a| a.key = Some(None));
    assert_eq!(program.serialized(), Err(ConvertError::Mesa(NIR_SERIALIZE)));
    answer(|a| a.key = Some(None));
    assert_eq!(program.key(), Err(ConvertError::Mesa(NIR_SERIALIZE)));
}

#[test]
fn the_common_form_is_asked_in_the_stages_set_with_the_documented_ranges_and_reflects_each_field() {
    for (stage, _, _) in STAGES {
        let program = SpirvProgram::read(&[], stage, "main", false).unwrap();
        calls();
        answer(|a| {
            a.common = Some(SpirvCommonForm {
                shader: Some(COMMON),
                bindings: one_binding_of_each_kind(),
                result: constants::BIND_OK,
            });
        });
        let (common, bindings) = program
            .to_common_form(stage)
            .unwrap_or_else(|_| panic!("made"));
        assert_eq!(
            calls(),
            [Call::CommonForm {
                shader: READ,
                set: documented_set(stage),
                ranges: documented_ranges()
            }],
            "{stage:?}"
        );
        assert_eq!(bindings, reflected_in(documented_set(stage)), "{stage:?}");
        answer(|a| a.flat = Some(Some(flat(5))));
        assert_eq!(common.flatten(), Ok(flat(5)));
        assert_eq!(calls(), [Call::CommonFlatten(COMMON)]);
    }
}

#[test]
fn a_common_forms_result_is_refused_by_its_documented_reason() {
    let cases = [
        (
            constants::BIND_BEYOND_LIMIT,
            Some(COMMON),
            ConvertError::Bindings(BEYOND_LIMIT),
        ),
        (
            constants::BIND_UNBOUNDED,
            Some(COMMON),
            ConvertError::Bindings(UNBOUNDED),
        ),
        (
            constants::BIND_OUT_OF_MEMORY,
            Some(COMMON),
            ConvertError::Bindings(OUT_OF_MEMORY),
        ),
        (
            4,
            Some(COMMON),
            ConvertError::Unknown("enum wc_bind_result", 4),
        ),
        (
            constants::BIND_OK,
            None,
            ConvertError::Bindings(OUT_OF_MEMORY),
        ),
    ];
    for (result, shader, refused) in cases {
        let program = SpirvProgram::read(&[], Stage::Compute, "main", false).unwrap();
        answer(|a| {
            a.common = Some(SpirvCommonForm {
                shader,
                bindings: Vec::new(),
                result,
            })
        });
        assert_eq!(
            program.to_common_form(Stage::Compute).map(|_| ()),
            Err(refused)
        );
    }
    let program = SpirvProgram::read(&[], Stage::Compute, "main", false).unwrap();
    let mut beyond = one_binding_of_each_kind();
    beyond[2].kind = 4;
    answer(|a| {
        a.common = Some(SpirvCommonForm {
            shader: Some(COMMON),
            bindings: beyond,
            result: 0,
        })
    });
    assert_eq!(
        program.to_common_form(Stage::Compute).map(|_| ()),
        Err(ConvertError::Unknown(SLOT_KIND, 4))
    );
    let program = SpirvProgram::read(&[], Stage::Compute, "main", false).unwrap();
    let (common, _) = program
        .to_common_form(Stage::Compute)
        .unwrap_or_else(|_| panic!("made"));
    answer(|a| a.flat = Some(None));
    assert_eq!(common.flatten(), Err(ConvertError::Mesa(FLATTEN)));
    calls();
}

#[test]
fn compile_hands_each_source_in_the_bridges_order_of_stages() {
    let stages = GlslStage::compile(&[
        (Stage::TessEvaluation, "e"),
        (Stage::Fragment, "f"),
        (Stage::Vertex, "v"),
    ])
    .unwrap_or_else(|_| panic!("compiled"));
    assert_eq!(
        calls(),
        [Call::GlslToNir([
            Some("v".into()),
            Some("f".into()),
            None,
            None,
            None,
            Some("e".into()),
        ])]
    );
    let order: Vec<Stage> = stages.iter().map(|s| s.stage).collect();
    assert_eq!(
        order,
        STAGES.map(|(s, _, _)| s),
        "every stage the compiler gave, in its order"
    );
}

#[test]
fn compile_refuses_a_source_with_a_nul_and_the_compilers_log_as_glsl() {
    let refused = GlslStage::compile(&[(Stage::Vertex, "a\0b")]).map(|_| ());
    assert_eq!(
        refused,
        Err(ConvertError::Glsl(
            "nul byte found in provided data at position: 1".into()
        ))
    );
    assert!(calls().is_empty());
    answer(|a| a.glsl = Some(Err(b"0:1: syntax error".to_vec())));
    let refused = GlslStage::compile(&[(Stage::Vertex, "x")]).map(|_| ());
    assert_eq!(refused, Err(ConvertError::Glsl("0:1: syntax error".into())));
    calls();
}

#[test]
fn each_name_keeps_every_field_its_stage_and_its_enumerations() {
    let kinds = [
        (constants::GLSL_UNIFORM, GlslNameKind::Uniform),
        (
            constants::GLSL_SUBROUTINE_UNIFORM,
            GlslNameKind::SubroutineUniform,
        ),
        (
            constants::GLSL_FRAG_COORD_Y_TRANSFORM,
            GlslNameKind::FragCoordYTransform,
        ),
        (
            constants::GLSL_POINT_COORD_Y_TRANSFORM,
            GlslNameKind::PointCoordYTransform,
        ),
        (constants::GLSL_OTHER_STATE, GlslNameKind::OtherState),
        (constants::GLSL_UNIFORM_BLOCK, GlslNameKind::UniformBlock),
        (constants::GLSL_STORAGE_BLOCK, GlslNameKind::StorageBlock),
        (constants::GLSL_SAMPLER, GlslNameKind::Sampler),
        (constants::GLSL_IMAGE, GlslNameKind::Image),
        (
            constants::GLSL_ATOMIC_COUNTER_BUFFER,
            GlslNameKind::AtomicCounterBuffer,
        ),
        (constants::GLSL_INPUT, GlslNameKind::Input),
        (constants::GLSL_OUTPUT, GlslNameKind::Output),
    ];
    let interpolations = [
        (
            constants::GLSL_INTERPOLATION_UNQUALIFIED,
            Interpolation::Unqualified,
        ),
        (constants::GLSL_INTERPOLATION_SMOOTH, Interpolation::Smooth),
        (constants::GLSL_INTERPOLATION_FLAT, Interpolation::Flat),
        (
            constants::GLSL_INTERPOLATION_NOPERSPECTIVE,
            Interpolation::NoPerspective,
        ),
        (constants::GLSL_INTERPOLATION_OTHER, Interpolation::Other),
    ];
    let mut names: [Vec<wcontainers_mesa_compiler::GlslName>; 6] = Default::default();
    let mut want: [Vec<GlslName>; 6] = Default::default();
    for (i, (raw, kind)) in kinds.into_iter().enumerate() {
        let (raw_interpolation, interpolation) = interpolations[i % interpolations.len()];
        let i = i as u32;
        let stage = (i % 6) as usize;
        names[stage].push(wcontainers_mesa_compiler::GlslName {
            kind: raw,
            name: Some(format!("n{i}").into_bytes()),
            type_: i.is_multiple_of(2).then(|| b"vec4".to_vec()),
            interpolation: raw_interpolation,
            at: 100 + i,
            count: 1 + i,
            element: 50 + i,
        });
        want[stage].push(GlslName {
            kind,
            name: format!("n{i}"),
            ty: i.is_multiple_of(2).then(|| "vec4".to_string()),
            interpolation,
            at: 100 + i,
            count: 1 + i,
            element: 50 + i,
        });
    }
    // No name is the empty name; bytes that are not UTF-8 are kept as replacement characters.
    names[1].push(wcontainers_mesa_compiler::GlslName {
        kind: constants::GLSL_INPUT,
        name: None,
        type_: Some(vec![0xff]),
        interpolation: constants::GLSL_INTERPOLATION_FLAT,
        at: 0,
        count: 0,
        element: 0,
    });
    want[1].push(GlslName {
        kind: GlslNameKind::Input,
        name: String::new(),
        ty: Some("\u{fffd}".into()),
        interpolation: Interpolation::Flat,
        at: 0,
        count: 0,
        element: 0,
    });
    answer(|a| {
        a.glsl = Some(Ok(GlslProgram {
            names,
            ..every_stage()
        }))
    });
    let stages = GlslStage::compile(&[(Stage::Vertex, "v")]).unwrap_or_else(|_| panic!("compiled"));
    calls();
    let got: Vec<Vec<GlslName>> = stages.into_iter().map(|s| s.names).collect();
    assert_eq!(got, want.to_vec());
}

#[test]
fn an_interpolation_outside_its_enumeration_is_refused_by_name() {
    let mut names: [Vec<wcontainers_mesa_compiler::GlslName>; 6] = Default::default();
    names[0].push(wcontainers_mesa_compiler::GlslName {
        kind: constants::GLSL_INPUT,
        name: None,
        type_: None,
        interpolation: constants::GLSL_INTERPOLATION_OTHER + 1,
        at: 0,
        count: 0,
        element: 0,
    });
    answer(|a| {
        a.glsl = Some(Ok(GlslProgram {
            names,
            ..every_stage()
        }))
    });
    let refused = GlslStage::compile(&[(Stage::Vertex, "v")]).map(|_| ());
    assert_eq!(
        refused,
        Err(ConvertError::Unknown(
            GLSL_INTERPOLATION,
            constants::GLSL_INTERPOLATION_OTHER + 1
        ))
    );
    calls();
    let mut names: [Vec<wcontainers_mesa_compiler::GlslName>; 6] = Default::default();
    names[0].push(wcontainers_mesa_compiler::GlslName {
        kind: constants::GLSL_OUTPUT + 1,
        name: None,
        type_: None,
        interpolation: 0,
        at: 0,
        count: 0,
        element: 0,
    });
    answer(|a| {
        a.glsl = Some(Ok(GlslProgram {
            names,
            ..every_stage()
        }))
    });
    let refused = GlslStage::compile(&[(Stage::Vertex, "v")]).map(|_| ());
    assert_eq!(
        refused,
        Err(ConvertError::Unknown(
            GLSL_NAME_KIND,
            constants::GLSL_OUTPUT + 1
        ))
    );
    calls();
}

#[test]
fn each_stage_is_prepared_by_its_own_step_and_keeps_its_stage() {
    let steps = [
        "vertex",
        "fragment",
        "compute",
        "geometry",
        "tess control",
        "tess evaluation",
    ];
    for ((stage, tag, _), step) in STAGES.into_iter().zip(steps) {
        let received = received(stage);
        let prepared = received
            .received
            .prepare()
            .unwrap_or_else(|_| panic!("prepared"));
        assert_eq!(calls(), [Call::Prepare { step, shader: tag }], "{stage:?}");
        assert_eq!(prepared.stage(), stage);
    }
}

/// A form whose every field holds a value of its own; `on` the value of every flag.
fn full_form(on: bool) -> PreparedForm<Tag, Tag> {
    PreparedForm {
        shader: Some(77),
        key: vec![4, 5, 6],
        streamed: vec![
            StreamOutput {
                buffer: 1,
                offset: 8,
                location: 3,
                position: true,
                first_component: 2,
                components: 3,
            },
            StreamOutput {
                buffer: 2,
                offset: 20,
                location: 6,
                position: false,
                first_component: 1,
                components: 4,
            },
        ],
        strides: [12, 16, 24, 36],
        streams_elsewhere: false,
        reads_sample_positions: on,
        writes_sample_mask: on,
        fetches_target: on,
        formatless_images: 0b1010_0000,
        integer_sampled_units: 0b0110,
        named_layer: 7,
        passes_through: true,
        pass_through: [Some(20), None, Some(22), None, Some(24)],
        generic_end: 9,
        geometry_vertices_out: 6,
        control_points_out: 3,
        makes_points: on,
    }
}

/// The tag of each made pass-through program, read through its key step's record.
fn pass_through_tags(prepared: &Prepared) -> Option<Vec<Option<Tag>>> {
    prepared.pass_through.as_ref().map(|entries| {
        entries
            .iter()
            .map(|made| {
                made.as_ref().map(|made| {
                    made.key().unwrap();
                    match calls().as_slice() {
                        [Call::MadeKey(tag)] => *tag,
                        other => panic!("{other:?}"),
                    }
                })
            })
            .collect()
    })
}

#[test]
fn a_prepared_program_carries_every_field_of_the_compilers_form() {
    for on in [false, true] {
        let prepared = prepared_with(Stage::Geometry, Some(full_form(on)));
        assert_eq!(prepared.key, key_of(&[4, 5, 6]));
        assert_eq!(
            prepared.streamed,
            Some(vec![
                StreamedOutput {
                    buffer: 1,
                    offset: 8,
                    location: 3,
                    position: true,
                    first_component: 2,
                    components: 3
                },
                StreamedOutput {
                    buffer: 2,
                    offset: 20,
                    location: 6,
                    position: false,
                    first_component: 1,
                    components: 4
                },
            ])
        );
        assert_eq!(prepared.strides, [12, 16, 24, 36]);
        assert_eq!(prepared.reads_sample_positions, on);
        assert_eq!(prepared.writes_sample_mask, on);
        assert_eq!(prepared.fetches_target, on);
        assert_eq!(prepared.formatless_images, 0b1010_0000);
        assert_eq!(prepared.integer_sampled_units, 0b0110);
        assert_eq!(prepared.named_layer, Some(7));
        assert_eq!(
            pass_through_tags(&prepared),
            Some(vec![Some(20), None, Some(22), None, Some(24)])
        );
        assert_eq!(prepared.generic_end, 9);
        assert_eq!(prepared.geometry_vertices_out, 6);
        assert_eq!(prepared.control_points_out, 3);
        assert_eq!(prepared.makes_points, on);
        prepared
            .geometry_points_as_squares(0, 1.0, 0)
            .map(|_| ())
            .unwrap_or_else(|_| panic!("squares"));
        assert!(
            matches!(calls().as_slice(), [Call::Squares { shader: 77, .. }]),
            "the form's program"
        );
    }
}

#[test]
fn a_form_streaming_elsewhere_naming_no_layer_or_passing_nothing_through_says_none() {
    let form = PreparedForm {
        streams_elsewhere: true,
        named_layer: u32::MAX,
        passes_through: false,
        ..full_form(true)
    };
    let prepared = prepared_with(Stage::Vertex, Some(form));
    assert_eq!(prepared.streamed, None);
    assert_eq!(prepared.named_layer, None);
    assert!(prepared.pass_through.is_none());
    let refused = received(Stage::Vertex);
    answer(|a| {
        a.prepared = Some(PreparedForm {
            shader: None,
            ..plain_form(0)
        })
    });
    assert_eq!(
        refused.received.prepare().map(|_| ()),
        Err(ConvertError::Mesa(NIR_SERIALIZE))
    );
    calls();
}

/// Settings with every field its own value, `samples` samples.
fn settings(samples: u32) -> DrawSettings {
    DrawSettings {
        transform: wcontainers_nir::mesa::PositionTransform {
            flip_y: true,
            half_z: false,
            reverse_z: true,
            fixed_point_size: false,
        },
        key_transform: wcontainers_nir::mesa::PositionTransform {
            flip_y: false,
            half_z: true,
            reverse_z: false,
            fixed_point_size: true,
        },
        fetch_unit: 5,
        fetch_multisampled: true,
        samples,
    }
}

/// What `settings(samples)` reaches the compiler as for a program binding in `set`.
fn bridged(samples: u32, set: u32) -> VariantSettings {
    VariantSettings {
        transform: PositionTransform {
            flip_y: true,
            half_z: false,
            reverse_z: true,
            fixed_point_size: false,
        },
        key_transform: PositionTransform {
            flip_y: false,
            half_z: true,
            reverse_z: false,
            fixed_point_size: true,
        },
        fetch_unit: 5,
        fetch_multisampled: true,
        single_sampled: samples <= 1,
        samples,
        set,
        ranges: documented_ranges(),
    }
}

/// A swizzle and a sampler with every field its own value, and what they reach the compiler as.
fn swizzle() -> wcontainers_nir::mesa::TextureSwizzle {
    wcontainers_nir::mesa::TextureSwizzle {
        unit: 3,
        swizzle: [Swizzle::Y, Swizzle::W, Swizzle::Zero, Swizzle::One],
    }
}
fn bridged_swizzle() -> TextureSwizzle {
    // `enum wc_swizzle`: x, y, z, w, zero, one.
    TextureSwizzle {
        unit: 3,
        swizzle: [1, 3, 4, 5],
    }
}
fn sampler() -> wcontainers_nir::mesa::NearestSampler {
    wcontainers_nir::mesa::NearestSampler {
        unit: 2,
        wrap: [Wrap::ClampToEdge, Wrap::MirroredRepeat, Wrap::ClampToBorder],
        mipmapped: true,
        min_lod: 1.5f32.to_bits(),
        max_lod: 7.25f32.to_bits(),
        border: [11, 12, 13, 14],
    }
}
fn bridged_sampler() -> NearestSampler {
    // `enum wc_wrap`: repeat, clamp to edge, mirrored repeat, clamp to border.
    NearestSampler {
        unit: 2,
        wrap: [1, 2, 3],
        mipmapped: true,
        min_lod: 1.5,
        max_lod: 7.25,
        border: [11, 12, 13, 14],
    }
}

#[test]
fn each_stages_variant_is_asked_of_its_own_step_with_every_setting_swizzle_and_sampler() {
    let steps = [
        "vertex",
        "fragment",
        "compute",
        "geometry",
        "tess control",
        "tess evaluation",
    ];
    for ((stage, tag, _), step) in STAGES.into_iter().zip(steps) {
        let program = prepared(stage);
        for samples in [0, 1, 2, 4] {
            program
                .variant(&settings(samples), &[swizzle()], &[sampler()])
                .map(|_| ())
                .unwrap_or_else(|_| panic!("made"));
            assert_eq!(
                calls(),
                [Call::Variant {
                    step,
                    shader: tag + PREPARED,
                    settings: bridged(samples, documented_set(stage)),
                    swizzles: vec![bridged_swizzle()],
                    samplers: vec![bridged_sampler()],
                }],
                "{stage:?} {samples}"
            );
        }
    }
}

#[test]
fn a_variant_carries_its_key_and_its_bindings_in_its_stages_set() {
    for (stage, _, _) in STAGES {
        let program = prepared(stage);
        answer(|a| {
            a.variant = Some(VariantForm {
                shader: Some(VARIANT),
                result: constants::VARIANT_MADE,
                key: vec![1, 2],
                bindings: one_binding_of_each_kind(),
            });
        });
        let variant = program
            .variant(&settings(1), &[], &[])
            .unwrap_or_else(|_| panic!("made"));
        calls();
        assert_eq!(variant.key, key_of(&[1, 2]));
        assert_eq!(
            variant.bindings,
            reflected_in(documented_set(stage)),
            "{stage:?}"
        );
        answer(|a| a.flat = Some(Some(flat(8))));
        let flattened = variant.flatten(3).unwrap_or_else(|_| panic!("flattened"));
        assert_eq!(
            calls(),
            [Call::Flatten {
                shader: VARIANT,
                points_in: 3
            }]
        );
        assert_eq!(flattened.flat, flat(8));
        assert_eq!(flattened.key, key_of(&[1, 2]));
        assert_eq!(flattened.bindings, reflected_in(documented_set(stage)));
    }
    let variant = prepared(Stage::Vertex)
        .variant(&settings(1), &[], &[])
        .unwrap_or_else(|_| panic!("made"));
    answer(|a| a.flat = Some(None));
    assert_eq!(
        variant.flatten(0).map(|_| ()),
        Err(ConvertError::Mesa(FLATTEN))
    );
    calls();
}

#[test]
fn a_variants_result_is_refused_by_its_documented_reason() {
    let cases = [
        (
            constants::VARIANT_GATHERS_INTEGER_CUBE,
            Some(VARIANT),
            ConvertError::Unsupported(
                "a gather of an integer cube map, whose faces' edges are not joined".into(),
            ),
        ),
        (
            constants::VARIANT_SAMPLES_UNKNOWN,
            Some(VARIANT),
            ConvertError::Unsupported(
                "reads at the positions of 4 samples, which are not known".into(),
            ),
        ),
        (
            constants::VARIANT_BEYOND_LIMIT,
            Some(VARIANT),
            ConvertError::Bindings(BEYOND_LIMIT),
        ),
        (
            constants::VARIANT_UNBOUNDED,
            Some(VARIANT),
            ConvertError::Bindings(UNBOUNDED),
        ),
        (
            constants::VARIANT_OUT_OF_MEMORY,
            Some(VARIANT),
            ConvertError::Bindings(OUT_OF_MEMORY),
        ),
        (6, Some(VARIANT), ConvertError::Unknown(VARIANT_RESULT, 6)),
        (
            constants::VARIANT_MADE,
            None,
            ConvertError::Bindings(OUT_OF_MEMORY),
        ),
    ];
    let program = prepared(Stage::Fragment);
    for (result, shader, refused) in cases {
        answer(|a| {
            a.variant = Some(VariantForm {
                shader,
                result,
                key: Vec::new(),
                bindings: Vec::new(),
            })
        });
        assert_eq!(
            program.variant(&settings(4), &[], &[]).map(|_| ()),
            Err(refused)
        );
    }
    let mut beyond = one_binding_of_each_kind();
    beyond[0].kind = 4;
    answer(|a| {
        a.variant = Some(VariantForm {
            bindings: beyond,
            ..made_variant()
        })
    });
    assert_eq!(
        program.variant(&settings(1), &[], &[]).map(|_| ()),
        Err(ConvertError::Unknown(SLOT_KIND, 4))
    );
    calls();
}

/// The squares' parts, by the tags of their programs.
fn squares_tags(
    squares: wcontainers_nir::mesa::Squares<Recorder>,
) -> (&'static str, Option<Tag>, Option<Tag>) {
    let tag = |made: Made| {
        made.key().unwrap();
        match calls().as_slice() {
            [Call::MadeKey(tag)] => *tag,
            other => panic!("{other:?}"),
        }
    };
    match squares {
        wcontainers_nir::mesa::Squares::OnePixel => ("one pixel", None, None),
        wcontainers_nir::mesa::Squares::Refused => ("refused", None, None),
        wcontainers_nir::mesa::Squares::Made { before, geometry } => {
            ("made", before.map(tag), Some(tag(geometry)))
        }
    }
}

#[test]
fn points_as_squares_ask_the_vertex_or_evaluation_step_with_the_location_and_largest_size() {
    for (stage, tag, _) in STAGES {
        let program = prepared(stage);
        let step = match stage {
            Stage::Vertex => "vertex",
            Stage::TessEvaluation => "tess evaluation",
            other => {
                let refused = program.points_as_squares(4, 64.0).map(|_| ());
                let why = format!("points drawn as squares, for a program of the {other:?} stage");
                assert_eq!(refused, Err(ConvertError::Unsupported(why)));
                assert!(calls().is_empty());
                continue;
            }
        };
        let squares = program
            .points_as_squares(4, 64.0)
            .unwrap_or_else(|_| panic!("asked"));
        assert_eq!(
            calls(),
            [Call::Squares {
                step,
                shader: tag + PREPARED,
                at: 4,
                largest: 64.0,
                block: None
            }]
        );
        assert_eq!(squares_tags(squares), ("one pixel", None, None));
    }
}

#[test]
fn a_geometry_programs_squares_ask_its_step_with_the_location_largest_size_and_block() {
    for (stage, tag, _) in STAGES {
        let program = prepared(stage);
        let asked = program.geometry_points_as_squares(2, 32.0, 5);
        if stage == Stage::Geometry {
            let squares = asked.unwrap_or_else(|_| panic!("asked"));
            assert_eq!(
                calls(),
                [Call::Squares {
                    step: "geometry",
                    shader: tag + PREPARED,
                    at: 2,
                    largest: 32.0,
                    block: Some(5)
                }]
            );
            assert_eq!(squares_tags(squares), ("one pixel", None, None));
        } else {
            let why = format!(
                "a geometry program's points drawn as squares, for a program of the {stage:?} stage"
            );
            assert_eq!(asked.map(|_| ()), Err(ConvertError::Unsupported(why)));
            assert!(calls().is_empty());
        }
    }
}

#[test]
fn squares_are_what_their_documented_kind_says() {
    let program = prepared(Stage::Vertex);
    let cases = [
        (
            constants::SQUARES_ONE_PIXEL,
            Some(30),
            Some(31),
            ("one pixel", None, None),
        ),
        (
            constants::SQUARES_REFUSED,
            Some(30),
            Some(31),
            ("refused", None, None),
        ),
        (
            constants::SQUARES_MADE,
            Some(30),
            Some(31),
            ("made", Some(30), Some(31)),
        ),
        (
            constants::SQUARES_MADE,
            None,
            Some(31),
            ("made", None, Some(31)),
        ),
    ];
    for (kind, before, geometry, want) in cases {
        answer(|a| {
            a.squares = Some(Squares {
                kind,
                sized: true,
                before,
                geometry,
            })
        });
        let squares = program
            .points_as_squares(0, 1.0)
            .unwrap_or_else(|_| panic!("squares"));
        calls();
        assert_eq!(squares_tags(squares), want);
    }
    answer(|a| {
        a.squares = Some(Squares {
            kind: 3,
            sized: false,
            before: None,
            geometry: Some(31),
        })
    });
    assert_eq!(
        program.points_as_squares(0, 1.0).map(|_| ()),
        Err(ConvertError::Unknown(SQUARES_KIND, 3))
    );
    calls();
}

#[test]
fn a_made_programs_key_layer_and_variant_are_asked_of_it() {
    answer(|a| {
        a.squares = Some(Squares {
            kind: constants::SQUARES_MADE,
            sized: true,
            before: None,
            geometry: Some(31),
        })
    });
    let wcontainers_nir::mesa::Squares::Made { geometry, .. } = prepared(Stage::Vertex)
        .points_as_squares(0, 1.0)
        .unwrap_or_else(|_| panic!("squares"))
    else {
        panic!("made");
    };
    calls();
    answer(|a| a.key = Some(Some(vec![3, 3])));
    assert_eq!(geometry.key(), Ok(key_of(&[3, 3])));
    assert_eq!(calls(), [Call::MadeKey(31)]);
    answer(|a| a.key = Some(None));
    assert_eq!(geometry.key(), Err(ConvertError::Mesa(NIR_SERIALIZE)));
    calls();
    answer(|a| a.named_layer = Some(7));
    assert_eq!(geometry.named_layer_location(), Some(7));
    assert_eq!(calls(), [Call::MadeNamedLayer(31)]);
    answer(|a| a.named_layer = Some(u32::MAX));
    assert_eq!(geometry.named_layer_location(), None);
    calls();
    for (stage, _, _) in STAGES {
        geometry
            .variant(stage, &settings(2), &[swizzle()], &[sampler()])
            .map(|_| ())
            .unwrap_or_else(|_| panic!("made"));
        assert_eq!(
            calls(),
            [Call::Variant {
                step: "made",
                shader: 31,
                settings: bridged(2, documented_set(stage)),
                swizzles: vec![bridged_swizzle()],
                samplers: vec![bridged_sampler()],
            }],
            "{stage:?}"
        );
    }
    answer(|a| {
        a.variant = Some(VariantForm {
            result: 6,
            ..made_variant()
        })
    });
    assert_eq!(
        geometry
            .variant(Stage::Geometry, &settings(1), &[], &[])
            .map(|_| ()),
        Err(ConvertError::Unknown(VARIANT_RESULT, 6))
    );
    calls();
}

#[test]
fn a_stage_received_through_the_gallium_trait_is_prepared_by_its_own_step() {
    use wcontainers_nir::mesa::{Received, Staged};
    let stages: [(Staged<Tag, Tag, Tag, Tag, Tag, Tag>, &str, Stage); 6] = [
        (Staged::Vertex(VERTEX), "vertex", Stage::Vertex),
        (Staged::Fragment(FRAGMENT), "fragment", Stage::Fragment),
        (Staged::Compute(COMPUTE), "compute", Stage::Compute),
        (Staged::Geometry(GEOMETRY), "geometry", Stage::Geometry),
        (Staged::TessControl(TESS_CONTROL), "tess control", Stage::TessControl),
        (Staged::TessEvaluation(TESS_EVALUATION), "tess evaluation", Stage::TessEvaluation),
    ];
    for ((shader, step, stage), (_, tag, _)) in stages.into_iter().zip(STAGES) {
        let prepared = Received::new(Recorder, shader)
            .prepare()
            .unwrap_or_else(|_| panic!("prepared"));
        assert_eq!(calls(), [Call::Prepare { step, shader: tag }], "{stage:?}");
        assert_eq!(prepared.stage(), stage);
    }
}
