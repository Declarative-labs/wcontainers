//! The start-up probe of what the host's engine does where WebAssembly leaves results open: whether its
//! relaxed fused multiply-add fuses, and whether its not-a-number results already follow Arm's rules. A
//! runner instantiates [`probe_module`], calls its one export, [`PROBE`], and gives the answer to
//! [`Host::from_probe`].

use crate::meaning::Host;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, InstructionSink, Module,
    TypeSection, ValType,
};

pub use wcontainers_guest_cpu::PROBE;

const FUSED: i32 = 1;
const ARM_NANS: i32 = 2;

impl Host {
    pub fn from_probe(answer: i32) -> Host {
        Host {
            fused_multiply_add: answer & FUSED != 0,
            arm_nans: answer & ARM_NANS != 0,
        }
    }

    /// The probe's answer this host gives back.
    pub fn answer(self) -> i32 {
        (i32::from(self.fused_multiply_add) * FUSED) | (i32::from(self.arm_nans) * ARM_NANS)
    }
}

fn splat32(bits: u32) -> i128 {
    let b = u128::from(bits);
    (b | b << 32 | b << 64 | b << 96) as i128
}

fn splat64(bits: u64) -> i128 {
    let b = u128::from(bits);
    (b | b << 64) as i128
}

/// One case: operands as v128 constants, the operation, and the bits Arm gives.
struct Case {
    double: bool,
    operands: Vec<u64>,
    op: fn(&mut InstructionSink<'_>, bool),
    expected: u64,
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
    exports.export(PROBE, ExportKind::Func, 0);
    module.section(&exports);

    let mut f = Function::new([]);
    let mut s = f.instructions();
    // Fused: (1 + 2^-30)(1 - 2^-30) - 1 is -2^-60 exactly, and 0 when the product is rounded first; in
    // single precision with 2^-13 likewise.
    let a64 = 1.0 + 2f64.powi(-30);
    let b64 = 1.0 - 2f64.powi(-30);
    s.v128_const(splat64(a64.to_bits()))
        .v128_const(splat64(b64.to_bits()))
        .v128_const(splat64((-1f64).to_bits()))
        .f64x2_relaxed_madd()
        .f64x2_extract_lane(0)
        .f64_const(0.0.into())
        .f64_ne();
    let a32 = 1.0 + 2f32.powi(-13);
    let b32 = 1.0 - 2f32.powi(-13);
    s.v128_const(splat32(a32.to_bits()))
        .v128_const(splat32(b32.to_bits()))
        .v128_const(splat32((-1f32).to_bits()))
        .f32x4_relaxed_madd()
        .f32x4_extract_lane(0)
        .f32_const(0.0.into())
        .f32_ne()
        .i32_and();
    // Not-a-numbers, each against the bits Arm gives.
    s.i32_const(1);
    for case in cases() {
        for &operand in &case.operands {
            s.v128_const(if case.double {
                splat64(operand)
            } else {
                splat32(operand as u32)
            });
        }
        (case.op)(&mut s, case.double);
        if case.double {
            s.i64x2_extract_lane(0)
                .i64_const(case.expected as i64)
                .i64_eq();
        } else {
            s.i32x4_extract_lane(0)
                .i32_const(case.expected as u32 as i32)
                .i32_eq();
        }
        s.i32_and();
    }
    s.i32_const(ARM_NANS).i32_mul().i32_or().end();
    let mut code = CodeSection::new();
    code.function(&f);
    module.section(&code);
    module.finish()
}

fn cases() -> Vec<Case> {
    let mut all = Vec::new();
    for double in [false, true] {
        let (s1, s2, q1, q2, one, inf, minus_one, default) = if double {
            (
                0x7ff0_0000_0000_0001u64,
                0xfff0_0000_0000_0002,
                0x7ff8_0000_0000_0003,
                0xfff8_0000_0000_0004,
                1f64.to_bits(),
                f64::INFINITY.to_bits(),
                (-1f64).to_bits(),
                0x7ff8_0000_0000_0000,
            )
        } else {
            (
                0x7f80_0001,
                0xff80_0002,
                0x7fc0_0003,
                0xffc0_0004,
                u64::from(1f32.to_bits()),
                u64::from(f32::INFINITY.to_bits()),
                u64::from((-1f32).to_bits()),
                0x7fc0_0000,
            )
        };
        let quiet = if double { 1 << 51 } else { 1 << 22 };
        let add: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_add()
            } else {
                s.f32x4_add()
            };
        };
        let sub: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_sub()
            } else {
                s.f32x4_sub()
            };
        };
        let mul: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_mul()
            } else {
                s.f32x4_mul()
            };
        };
        let div: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_div()
            } else {
                s.f32x4_div()
            };
        };
        let max: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_max()
            } else {
                s.f32x4_max()
            };
        };
        let min: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_min()
            } else {
                s.f32x4_min()
            };
        };
        let sqrt: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_sqrt()
            } else {
                s.f32x4_sqrt()
            };
        };
        let nearest: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_nearest()
            } else {
                s.f32x4_nearest()
            };
        };
        let floor: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_floor()
            } else {
                s.f32x4_floor()
            };
        };
        // Operands in WebAssembly's order: a * b + c, where Arm takes c (the addend) first.
        let madd: fn(&mut InstructionSink<'_>, bool) = |s, d| {
            if d {
                s.f64x2_relaxed_madd()
            } else {
                s.f32x4_relaxed_madd()
            };
        };
        let case = |operands: Vec<u64>, op, expected| Case {
            double,
            operands,
            op,
            expected,
        };
        all.extend([
            case(vec![s1, q2], add, s1 | quiet),
            case(vec![q1, s2], add, s2 | quiet),
            case(vec![q1, q2], add, q1),
            case(vec![one, q2], sub, q2),
            case(vec![inf, inf], sub, default),
            case(vec![0, inf], mul, default),
            case(vec![s2, one], mul, s2 | quiet),
            case(vec![0, 0], div, default),
            case(vec![q2, s1], max, s1 | quiet),
            case(vec![q1, q2], min, q1),
            case(vec![minus_one], sqrt, default),
            case(vec![s2], nearest, s2 | quiet),
            case(vec![q1], floor, q1),
            case(vec![q1, q2, s1], madd, s1 | quiet),
            case(vec![q1, q2, one], madd, q1),
            case(vec![0, inf, q2], madd, default),
            case(vec![one, s2, q1], madd, s2 | quiet),
        ]);
    }
    all
}
