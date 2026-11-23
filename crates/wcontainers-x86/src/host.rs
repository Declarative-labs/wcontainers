//! The start-up probe of what the host's engine does where WebAssembly leaves results open: whether its
//! relaxed fused multiply-add fuses, and whether its not-a-number results already follow x86's rules (the first operand's quieted, then the second's, else the
//! negative default one; conversions and roundings keep the payload). A runner instantiates
//! [`probe_module`], calls its one export, and gives the answer to [`Host::from_probe`].

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, InstructionSink, Module,
    TypeSection, ValType,
};

/// What the host's engine does, in the cache key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Host {
    pub x86_nans: bool,
    pub fused_multiply_add: bool,
}

const X86_NANS: i32 = 1;
const FUSED: i32 = 2;

impl Host {
    pub fn from_probe(answer: i32) -> Host {
        Host {
            x86_nans: answer & X86_NANS != 0,
            fused_multiply_add: answer & FUSED != 0,
        }
    }

    pub fn answer(self) -> i32 {
        (i32::from(self.x86_nans) * X86_NANS) | (i32::from(self.fused_multiply_add) * FUSED)
    }
}

pub(crate) use crate::{DEFAULT32, QUIET32};
/// The quiet bit, and x86's default not-a-number, negative and quiet, of double precision.
pub(crate) const QUIET64: u64 = 1 << (f64::MANTISSA_DIGITS - 2);
pub(crate) const DEFAULT64: u64 = (-f64::INFINITY).to_bits() | QUIET64;
/// Not-a-numbers with payloads: signaling and quiet, of either sign.
const SIGNALING32: u32 = f32::INFINITY.to_bits() | 1;
const QUIET_PAYLOAD32: u32 = (-f32::INFINITY).to_bits() | QUIET32 | 2;
const SIGNALING64: u64 = f64::INFINITY.to_bits() | 1;
const QUIET_PAYLOAD64: u64 = (-f64::INFINITY).to_bits() | QUIET64 | 2;

pub(crate) fn splat32(bits: u32) -> i128 {
    let b = u128::from(bits);
    (0..u128::BITS / u32::BITS).fold(0, |v, n| v | b << (n * u32::BITS)) as i128
}

pub(crate) fn splat64(bits: u64) -> i128 {
    let b = u128::from(bits);
    (b | b << u64::BITS) as i128
}

/// One case: the operand vectors, the operation, and lane 0's bits as x86 gives them.
struct Case {
    operands: Vec<i128>,
    op: fn(&mut InstructionSink<'_>),
    expected: u64,
}

fn cases() -> Vec<Case> {
    let one32 = 1f32.to_bits();
    let one64 = 1f64.to_bits();
    let infinity32 = f32::INFINITY.to_bits();
    let infinity64 = f64::INFINITY.to_bits();
    let case = |operands: &[i128], op: fn(&mut InstructionSink<'_>), expected: u64| Case {
        operands: operands.to_vec(),
        op,
        expected,
    };
    vec![
        // The first operand's, quieted, even when the second is signaling.
        case(
            &[splat32(QUIET_PAYLOAD32), splat32(SIGNALING32)],
            |s| {
                s.f32x4_add();
            },
            QUIET_PAYLOAD32.into(),
        ),
        case(
            &[splat32(SIGNALING32), splat32(one32)],
            |s| {
                s.f32x4_mul();
            },
            (SIGNALING32 | QUIET32).into(),
        ),
        case(
            &[splat32(one32), splat32(SIGNALING32)],
            |s| {
                s.f32x4_sub();
            },
            (SIGNALING32 | QUIET32).into(),
        ),
        case(
            &[splat32(infinity32), splat32(infinity32)],
            |s| {
                s.f32x4_sub();
            },
            DEFAULT32.into(),
        ),
        case(
            &[splat32(0), splat32(0)],
            |s| {
                s.f32x4_div();
            },
            DEFAULT32.into(),
        ),
        case(
            &[splat32((-1f32).to_bits())],
            |s| {
                s.f32x4_sqrt();
            },
            DEFAULT32.into(),
        ),
        case(
            &[splat32(SIGNALING32)],
            |s| {
                s.f32x4_nearest();
            },
            (SIGNALING32 | QUIET32).into(),
        ),
        case(
            &[splat64(QUIET_PAYLOAD64), splat64(SIGNALING64)],
            |s| {
                s.f64x2_div();
            },
            QUIET_PAYLOAD64,
        ),
        case(
            &[splat64(infinity64), splat64(0)],
            |s| {
                s.f64x2_mul();
            },
            DEFAULT64,
        ),
        case(
            &[splat64(one64), splat64(SIGNALING64)],
            |s| {
                s.f64x2_add();
            },
            SIGNALING64 | QUIET64,
        ),
        case(
            &[splat64(SIGNALING64)],
            |s| {
                s.f64x2_sqrt();
            },
            SIGNALING64 | QUIET64,
        ),
        // Conversions keep the payload's top bits.
        case(
            &[splat32(SIGNALING32)],
            |s| {
                s.f64x2_promote_low_f32x4();
            },
            infinity64 | QUIET64 | 1 << (f64::MANTISSA_DIGITS - f32::MANTISSA_DIGITS),
        ),
        case(
            &[splat64(
                QUIET_PAYLOAD64 | 1 << (f64::MANTISSA_DIGITS - f32::MANTISSA_DIGITS),
            )],
            |s| {
                s.f32x4_demote_f64x2_zero();
            },
            ((-f32::INFINITY).to_bits() | QUIET32 | 1).into(),
        ),
    ]
}

pub fn probe_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function([], [ValType::I32]);
    module.section(&types);
    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);
    let mut exports = ExportSection::new();
    exports.export(wcontainers_guest_cpu::PROBE, ExportKind::Func, 0);
    module.section(&exports);
    let mut f = Function::new([]);
    let mut s = f.instructions();
    s.i32_const(X86_NANS);
    for case in cases() {
        for &operand in &case.operands {
            s.v128_const(operand);
        }
        (case.op)(&mut s);
        s.i64x2_extract_lane(0);
        // Single precision answers are lane 0's low half.
        if case.expected >> u32::BITS == 0 {
            s.i64_const(u32::MAX.into()).i64_and();
        }
        s.i64_const(case.expected as i64).i64_eq().i32_and();
    }
    // Fused: with e = 2^-(p/2 + 2), (1 + e)(1 - e) - 1 is -e^2 exactly, and 0 when the product is rounded first,
    // as e^2 is below half a unit of 1.
    let e64 = 2f64.powi(-(f64::MANTISSA_DIGITS as i32 / 2 + 2));
    let e32 = 2f32.powi(-(f32::MANTISSA_DIGITS as i32 / 2 + 2));
    let (a64, b64, a32, b32) = (1.0 + e64, 1.0 - e64, 1.0 + e32, 1.0 - e32);
    s.v128_const(splat64(a64.to_bits()))
        .v128_const(splat64(b64.to_bits()))
        .v128_const(splat64((-1f64).to_bits()))
        .f64x2_relaxed_madd()
        .f64x2_extract_lane(0)
        .f64_const(0.0.into())
        .f64_ne()
        .v128_const(splat32(a32.to_bits()))
        .v128_const(splat32(b32.to_bits()))
        .v128_const(splat32((-1f32).to_bits()))
        .f32x4_relaxed_madd()
        .f32x4_extract_lane(0)
        .f32_const(0.0.into())
        .f32_ne()
        .i32_and()
        .i32_const(FUSED)
        .i32_mul()
        .i32_or();
    s.end();
    let mut code = CodeSection::new();
    code.function(&f);
    module.section(&code);
    module.finish()
}
