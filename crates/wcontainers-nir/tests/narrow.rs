//! Integer operations route B carries in other operations, held to lavapipe running the same
//! program natively and to Rust's arithmetic. Mesa's alpha-to-coverage pass's 16-bit operations,
//! carried in 32-bit words (plan 15.4), over every 16-bit value and every 32-bit shift count below
//! 32: the second operand and the bits above each input's low 16 are seeded (`WC_NIR_SEED`), and
//! every value and count is run, so there is no count of cases to choose. SPIR-V's extended
//! multiplications, made of 32-bit ones by Mesa, on seeded operands (`WC_NIR_SEED`, and
//! `WC_NIR_INPUT_SETS` cases) and the edges of both signs.
#![cfg(mesa)]

#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one compute program through it"
)]
#[path = "corpus/compare.rs"]
mod compare;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one compute program through it"
)]
#[path = "corpus/interface.rs"]
mod interface;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one compute program through it"
)]
#[path = "corpus/lavapipe.rs"]
mod lavapipe;

use ash::vk;
use lavapipe::{Descriptor, Element, Job, Lavapipe};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use spirv::{
    AddressingModel, BuiltIn, Capability, Decoration, ExecutionMode, ExecutionModel, MemoryModel,
    Op, StorageClass,
};
use wcontainers_nir::mesa::{Nir, Stage};
use wcontainers_nir::spirv::{WORD_COUNT_SHIFT, validate};
use wcontainers_shaders::ENTRY_POINT;

/// Every count a shift is run with: all below the width of its 32-bit count.
const SHIFT_COUNTS: u32 = u32::BITS;
/// One invocation a 16-bit value and shift count.
const INVOCATIONS: u32 = (1 << u16::BITS) * SHIFT_COUNTS;
/// The extended multiplications' cases when `WC_NIR_INPUT_SETS` is unset.
const MULTIPLICATIONS: u32 = 1 << u16::BITS;
/// A multiplication's input words (the two operands) and results (the unsigned product's low and
/// high words, then the signed product's).
const MULTIPLICATION_INPUTS: u32 = 2;
const MULTIPLICATION_RESULTS: u32 = 4;
/// Operands every multiplication run takes besides the seeded ones, as 32-bit words: the edges of
/// both signs.
const EDGES: [u32; 6] = [
    0,
    1,
    u32::MAX,
    i32::MAX as u32,
    i32::MIN as u32,
    1 << u16::BITS,
];
const WORKGROUP_SIZE: u32 = 64;
/// An invocation's input words, in order: the value, the second operand (each with seeded upper
/// bits) and the shift count.
const INPUT_WORDS: u32 = 3;
const SHIFT_WORD: u32 = 2;
/// An invocation's results, in order: the value through u2u16, v*w, v*v, v&w, v|w, v<<s, v>>s.
const RESULTS: u32 = 7;
/// The components of `GlobalInvocationId`.
const INVOCATION_ID_COMPONENTS: u32 = 3;
/// How many differing results the failure names.
const NAMED_DIFFERENCES: usize = 8;

/// The results Rust's 16-bit arithmetic gives, in the program's order.
fn expected(value: u16, other: u16, count: u32) -> [u32; RESULTS as usize] {
    [
        value,
        value.wrapping_mul(other),
        value.wrapping_mul(value),
        value & other,
        value | other,
        value.wrapping_shl(count),
        value.wrapping_shr(count),
    ]
    .map(u32::from)
}

/// A SPIR-V module written word by word.
#[derive(Default)]
struct Module {
    bound: u32,
    words: Vec<u32>,
}

impl Module {
    fn id(&mut self) -> u32 {
        self.bound += 1;
        self.bound
    }

    fn op(&mut self, op: Op, operands: &[u32]) {
        let count = u32::try_from(operands.len() + 1).expect("a short instruction");
        self.words.push((count << WORD_COUNT_SHIFT) | op as u32);
        self.words.extend_from_slice(operands);
    }

    /// An instruction with a result of `ty`.
    fn value(&mut self, op: Op, ty: u32, operands: &[u32]) -> u32 {
        let id = self.id();
        let mut all = vec![ty, id];
        all.extend_from_slice(operands);
        self.op(op, &all);
        id
    }
}

/// A literal string's words: its bytes, a zero after them, padded to a word.
fn string(text: &str) -> Vec<u32> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes.resize(bytes.len().next_multiple_of(size_of::<u32>()), 0);
    bytes
        .chunks(size_of::<u32>())
        .map(|c| u32::from_le_bytes(c.try_into().expect("a word")))
        .collect()
}

/// A compute program in which each invocation reads `input_words` words from buffer 0 and writes
/// the words `body` makes of them to buffer 1. `types` declares what `body` needs, given the
/// 32-bit unsigned type.
fn program<T: Copy>(
    capabilities: &[Capability],
    input_words: u32,
    types: impl FnOnce(&mut Module, u32) -> T,
    body: impl FnOnce(&mut Module, T, u32, &[u32]) -> Vec<u32>,
) -> Vec<u32> {
    let mut m = Module::default();
    let [
        void,
        function,
        uint,
        uvec3,
        input_pointer,
        invocation,
        words,
        block,
    ] = std::array::from_fn(|_| m.id());
    let [block_pointer, inputs, outputs, word_pointer, main] = std::array::from_fn(|_| m.id());
    m.op(Op::Capability, &[Capability::Shader as u32]);
    for &c in capabilities {
        m.op(Op::Capability, &[c as u32]);
    }
    m.op(
        Op::MemoryModel,
        &[AddressingModel::Logical as u32, MemoryModel::GLSL450 as u32],
    );
    let mut entry = vec![ExecutionModel::GLCompute as u32, main];
    entry.extend(string(ENTRY_POINT));
    entry.push(invocation);
    m.op(Op::EntryPoint, &entry);
    m.op(
        Op::ExecutionMode,
        &[main, ExecutionMode::LocalSize as u32, WORKGROUP_SIZE, 1, 1],
    );
    m.op(
        Op::Decorate,
        &[
            invocation,
            Decoration::BuiltIn as u32,
            BuiltIn::GlobalInvocationId as u32,
        ],
    );
    let word_bytes = u32::try_from(size_of::<u32>()).expect("a small size");
    m.op(
        Op::Decorate,
        &[words, Decoration::ArrayStride as u32, word_bytes],
    );
    m.op(
        Op::MemberDecorate,
        &[block, 0, Decoration::Offset as u32, 0],
    );
    m.op(Op::Decorate, &[block, Decoration::Block as u32]);
    for (binding, buffer) in [inputs, outputs].into_iter().enumerate() {
        let binding = u32::try_from(binding).expect("two");
        m.op(Op::Decorate, &[buffer, Decoration::DescriptorSet as u32, 0]);
        m.op(Op::Decorate, &[buffer, Decoration::Binding as u32, binding]);
    }
    m.op(Op::TypeVoid, &[void]);
    m.op(Op::TypeFunction, &[function, void]);
    m.op(Op::TypeInt, &[uint, u32::BITS, 0]);
    let declared = types(&mut m, uint);
    m.op(Op::TypeVector, &[uvec3, uint, INVOCATION_ID_COMPONENTS]);
    m.op(
        Op::TypePointer,
        &[input_pointer, StorageClass::Input as u32, uvec3],
    );
    m.op(
        Op::Variable,
        &[input_pointer, invocation, StorageClass::Input as u32],
    );
    m.op(Op::TypeRuntimeArray, &[words, uint]);
    m.op(Op::TypeStruct, &[block, words]);
    m.op(
        Op::TypePointer,
        &[block_pointer, StorageClass::StorageBuffer as u32, block],
    );
    for buffer in [inputs, outputs] {
        m.op(
            Op::Variable,
            &[block_pointer, buffer, StorageClass::StorageBuffer as u32],
        );
    }
    m.op(
        Op::TypePointer,
        &[word_pointer, StorageClass::StorageBuffer as u32, uint],
    );
    // Enough constants to index every input word and every result; the results' count is known
    // only after `body`, so the input words' constants are made first and the rest as needed.
    let mut constants: Vec<u32> = (0..=input_words)
        .map(|value| m.value(Op::Constant, uint, &[value]))
        .collect();

    let mut function_words = Module {
        bound: m.bound,
        words: Vec::new(),
    };
    let f = &mut function_words;
    f.op(Op::Function, &[void, main, 0, function]);
    let label = f.id();
    f.op(Op::Label, &[label]);
    let id = f.value(Op::Load, uvec3, &[invocation]);
    let id = f.value(Op::CompositeExtract, uint, &[id, 0]);
    let first_input = f.value(Op::IMul, uint, &[id, constants[input_words as usize]]);
    let loaded: Vec<u32> = (0..input_words)
        .map(|at| {
            let index = f.value(Op::IAdd, uint, &[first_input, constants[at as usize]]);
            let pointer = f.value(
                Op::AccessChain,
                word_pointer,
                &[inputs, constants[0], index],
            );
            f.value(Op::Load, uint, &[pointer])
        })
        .collect();
    let results = body(f, declared, uint, &loaded);
    let result_words = u32::try_from(results.len()).expect("a few results");
    m.bound = f.bound;
    for value in u32::try_from(constants.len()).expect("a few")..=result_words {
        constants.push(m.value(Op::Constant, uint, &[value]));
    }
    m.words.append(&mut function_words.words);
    let first_result = m.value(Op::IMul, uint, &[id, constants[result_words as usize]]);
    for (at, wide) in results.into_iter().enumerate() {
        let index = m.value(Op::IAdd, uint, &[first_result, constants[at]]);
        let pointer = m.value(
            Op::AccessChain,
            word_pointer,
            &[outputs, constants[0], index],
        );
        m.op(Op::Store, &[pointer, wide]);
    }
    m.op(Op::Return, &[]);
    m.op(Op::FunctionEnd, &[]);

    let (major, minor) = compare::SPIRV_VERSION;
    let version = (u32::from(major) << u16::BITS) | (u32::from(minor) << u8::BITS);
    let mut module = vec![spirv::MAGIC_NUMBER, version, 0, m.bound + 1, 0];
    module.extend(m.words);
    module
}

/// The 16-bit program: each invocation reads its value, operand and count and writes its results,
/// each a 16-bit operation widened to 32 bits.
fn sixteen_bit_program() -> Vec<u32> {
    program(
        &[Capability::Int16],
        INPUT_WORDS,
        |m, _| {
            let short = m.id();
            m.op(Op::TypeInt, &[short, u16::BITS, 0]);
            short
        },
        |m, short, uint, inputs| {
            let value = m.value(Op::UConvert, short, &[inputs[0]]);
            let other = m.value(Op::UConvert, short, &[inputs[1]]);
            let count = inputs[SHIFT_WORD as usize];
            let results = [
                value,
                m.value(Op::IMul, short, &[value, other]),
                m.value(Op::IMul, short, &[value, value]),
                m.value(Op::BitwiseAnd, short, &[value, other]),
                m.value(Op::BitwiseOr, short, &[value, other]),
                m.value(Op::ShiftLeftLogical, short, &[value, count]),
                m.value(Op::ShiftRightLogical, short, &[value, count]),
            ];
            results
                .into_iter()
                .map(|r| m.value(Op::UConvert, uint, &[r]))
                .collect()
        },
    )
}

/// The extended multiplications' program: each invocation reads two operands and writes the low
/// and high words of their unsigned product, then of their signed one.
fn multiplication_program() -> Vec<u32> {
    program(
        &[],
        MULTIPLICATION_INPUTS,
        |m, uint| {
            let pair = m.id();
            m.op(Op::TypeStruct, &[pair, uint, uint]);
            pair
        },
        |m, pair, uint, inputs| {
            let mut results = Vec::new();
            for op in [Op::UMulExtended, Op::SMulExtended] {
                let product = m.value(op, pair, &[inputs[0], inputs[1]]);
                for half in [0, 1] {
                    results.push(m.value(Op::CompositeExtract, uint, &[product, half]));
                }
            }
            results
        },
    )
}

fn descriptors(inputs: &[u8], result_bytes: usize, places: [(u32, u32); 2]) -> Vec<Descriptor> {
    let results = vec![0; result_bytes];
    places
        .into_iter()
        .zip([inputs.to_vec(), results])
        .map(|((set, binding), bytes)| Descriptor {
            set,
            binding,
            kind: vk::DescriptorType::STORAGE_BUFFER,
            elements: vec![Element::Buffer(bytes)],
        })
        .collect()
}

/// The program's output words run natively by lavapipe and through route B, on `inputs` of
/// `invocations` invocations writing `result_words` words each.
fn run_both(
    program: &[u32],
    inputs: &[u32],
    invocations: u32,
    result_words: u32,
) -> (Vec<u32>, Vec<u32>) {
    let inputs: Vec<u8> = inputs.iter().flat_map(|w| w.to_le_bytes()).collect();
    let result_bytes = (invocations * result_words) as usize * size_of::<u32>();
    let mut nir = Nir::from_spirv(program, Stage::Compute, ENTRY_POINT).expect("NIR");
    let reflection = nir.to_common_form(Stage::Compute).expect("the common form");
    let module = wcontainers_nir::direct::module(&mut nir).expect("route B's module");
    validate(&module).expect("a module of the common form");
    let entry = &module.entry_points[0].name;
    let route = compare::write_spirv(
        &module,
        naga::ShaderStage::Compute,
        entry,
        naga::back::spv::WriterFlags::empty(),
    )
    .expect("naga writes the module");
    let place = |register: u32| {
        let r = reflection
            .iter()
            .find(|r| r.register == register)
            .expect("each buffer is reflected");
        (r.set, r.binding)
    };

    let lavapipe = Lavapipe::open().expect("lavapipe");
    let reference_descriptors = descriptors(&inputs, result_bytes, [(0, 0), (0, 1)]);
    let job = Job {
        stage: lavapipe::Stage::Compute,
        words: program,
        entry: ENTRY_POINT,
        descriptors: &reference_descriptors,
        immediates: &[],
        groups: [invocations / WORKGROUP_SIZE, 1, 1],
        vertices: 0,
        attributes: &[],
        feeder: None,
        capture_stride: 0,
        targets: &[],
        depth: false,
        clear: [0; 4],
    };
    let native = lavapipe.run(&job).expect("the native program runs");
    let route_descriptors = descriptors(&inputs, result_bytes, [place(0), place(1)]);
    let carried = lavapipe
        .run(&Job {
            words: &route,
            entry,
            descriptors: &route_descriptors,
            ..job
        })
        .expect("route B's module runs");
    (words(&native.storage[1]), words(&carried.storage[1]))
}

/// The seed every case is drawn from.
fn seed() -> u64 {
    std::env::var(compare::SEED).map_or(compare::DEFAULT_SEED, |text| {
        text.parse().expect("the seed is a number")
    })
}

/// A word's low 16 bits.
fn low(word: u32) -> u16 {
    u16::try_from(word & u32::from(u16::MAX)).expect("16 bits")
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks(size_of::<u32>())
        .map(|c| u32::from_le_bytes(c.try_into().expect("a word")))
        .collect()
}

#[test]
fn sixteen_bit_operations_in_words_give_sixteen_bit_results() {
    let seed = seed();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut cases = Vec::new();
    for value in 0..=u16::MAX {
        for count in 0..SHIFT_COUNTS {
            let upper = |rng: &mut ChaCha8Rng| rng.random::<u32>() << u16::BITS;
            let other: u16 = rng.random();
            cases.push([
                u32::from(value) | upper(&mut rng),
                u32::from(other) | upper(&mut rng),
                count,
            ]);
        }
    }
    let inputs: Vec<u32> = cases.iter().flatten().copied().collect();
    let (native, widened) = run_both(&sixteen_bit_program(), &inputs, INVOCATIONS, RESULTS);

    let mut differences = Vec::new();
    for (case, [value, other, count]) in cases.iter().enumerate() {
        let want = expected(low(*value), low(*other), *count);
        let at = case * RESULTS as usize;
        for (result, &want) in want.iter().enumerate() {
            let (n, w) = (native[at + result], widened[at + result]);
            if n != want || w != want {
                differences.push(format!(
                    "value {:#x} operand {:#x} count {count} result {result}: Rust {want:#x}, lavapipe {n:#x}, route B {w:#x}",
                    low(*value),
                    low(*other)
                ));
            }
        }
    }
    eprintln!(
        "narrow: {} cases ({} results), seed {seed:#x}, {} differ",
        cases.len(),
        cases.len() * RESULTS as usize,
        differences.len()
    );
    differences.truncate(NAMED_DIFFERENCES);
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn extended_multiplications_of_32_bit_words_give_64_bit_products() {
    let seed = seed();
    let asked: u32 = std::env::var(compare::INPUT_SETS).map_or(MULTIPLICATIONS, |text| {
        text.parse().expect("the count is a number")
    });
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut cases: Vec<[u32; MULTIPLICATION_INPUTS as usize]> =
        EDGES.iter().flat_map(|&a| EDGES.map(|b| [a, b])).collect();
    let invocations =
        (u32::try_from(cases.len()).expect("a few edges") + asked).next_multiple_of(WORKGROUP_SIZE);
    while cases.len() < invocations as usize {
        cases.push(rng.random());
    }
    let inputs: Vec<u32> = cases.iter().flatten().copied().collect();
    let (native, carried) = run_both(
        &multiplication_program(),
        &inputs,
        invocations,
        MULTIPLICATION_RESULTS,
    );
    let halves = |product: u64| [product as u32, (product >> u32::BITS) as u32];
    let mut differences = Vec::new();
    for (case, &[a, b]) in cases.iter().enumerate() {
        let unsigned = halves(u64::from(a) * u64::from(b));
        let signed = halves((i64::from(a as i32) * i64::from(b as i32)) as u64);
        let at = case * MULTIPLICATION_RESULTS as usize;
        for (result, want) in unsigned.into_iter().chain(signed).enumerate() {
            let (n, c) = (native[at + result], carried[at + result]);
            if n != want || c != want {
                differences.push(format!(
                    "{a:#x} times {b:#x} result {result}: Rust {want:#x}, lavapipe {n:#x}, route B {c:#x}"
                ));
            }
        }
    }
    eprintln!(
        "multiplications: {} cases ({} edge pairs), seed {seed:#x}, {} differ",
        cases.len(),
        EDGES.len() * EDGES.len(),
        differences.len()
    );
    differences.truncate(NAMED_DIFFERENCES);
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
