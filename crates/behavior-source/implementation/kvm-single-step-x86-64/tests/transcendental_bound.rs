//! The x87 transcendental forms' results on this host's chip lie within the bound the manual states of the value the
//! model works out before it rounds: under one unit in the last place rounding to nearest, under one and a half
//! otherwise. The behaviors leave these results out of what they compare; this test is the one place their values
//! are checked, one-sided: only the chip's distance from the exact value, never its last bit.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_x86_64::x87_transcendental::{Function, find_unrounded};
use wcontainers_processor_architecture_x86_64::{Instruction, X86_64};

const FORMS: [(Instruction, Function); 8] = [
    (Instruction::F2xm1, Function::TwoToXMinusOne),
    (Instruction::Fsin, Function::Sine),
    (Instruction::Fcos, Function::Cosine),
    (Instruction::Fptan, Function::Tangent),
    (Instruction::Fsincos, Function::SineCosine),
    (Instruction::Fpatan, Function::ArcTangent),
    (Instruction::Fyl2x, Function::YLogX),
    (Instruction::Fyl2xp1, Function::YLogXPlusOne),
];
/// Random arguments of each form under each rounding control.
const RANDOM_ARGUMENTS: usize = 400;
const ONE: u128 = 0x3FFF_8000_0000_0000_0000;
/// FSW's and FCW's OE and UE.
const OVERFLOW: u16 = 1 << 3;
const UNDERFLOW: u16 = 1 << 4;
const INITIAL_CONTROL: u16 = 0x037F;
/// The smallest exponent of a unit in the last place: a denormal's.
const SMALLEST_UNIT: i32 = -16382 - 63;

/// The chip's distance from the exact value, in units of the exact value's last place: the value an extended value
/// holds against a significand of 128 bits times 2 to `scale`.
fn measure_error(chip: u128, (negative, scale, significand): (bool, i32, u128)) -> f64 {
    let place = (scale + 127 - 63).max(SMALLEST_UNIT);
    // Both counted in units of 2 to the -62 of that place.
    let unit = place - 62;
    let shift = |bits: u128, exponent: i32| -> Option<u128> {
        let by = exponent - unit;
        if by >= 0 { bits.checked_shl(by as u32).filter(|shifted| shifted >> by == bits) } else { Some(bits.checked_shr((-by) as u32).unwrap_or(0)) }
    };
    let exact = shift(significand, scale).expect("the exact value at its own place");
    let biased = ((chip >> 64) & 0x7FFF) as i32;
    let Some(observed) = shift(u128::from(chip as u64), biased.max(1) - 16383 - 63) else {
        return f64::INFINITY;
    };
    let same_sign = (chip >> 79 != 0) == negative;
    let distance = if same_sign { observed.abs_diff(exact) } else { observed + exact };
    distance as f64 / (1u64 << 62) as f64
}

/// A random extended value of the sign and of a biased exponent in the range, from the generator's state.
fn make_value(state: &mut u64, exponents: (i32, i32), negative: bool) -> u128 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    let span = (exponents.1 - exponents.0 + 1) as u64;
    let exponent = exponents.0 + (*state % span) as i32;
    let significand = *state | 1 << 63;
    u128::from(negative) << 79 | ((exponent + 16383) as u128) << 64 | u128::from(significand)
}

/// The exponents of each operand of the form within the range the manual defines: ST(0)'s, then ST(1)'s.
fn find_exponents(function: Function) -> [(i32, i32); 2] {
    match function {
        Function::TwoToXMinusOne => [(-80, -1), (0, 0)],
        Function::Sine | Function::Cosine | Function::Tangent | Function::SineCosine => [(-70, 62), (0, 0)],
        Function::ArcTangent => [(-300, 300), (-300, 300)],
        Function::YLogX => [(-2000, 2000), (-60, 60)],
        Function::YLogXPlusOne => [(-80, -3), (-60, 60)],
    }
}

#[test]
fn transcendental_results_lie_within_the_manuals_bound() {
    let generator = CriticalValueCut::new(X86_64InputDomain);
    let mut machine = SingleStepMachine::new().expect("a machine");
    let mut random = 0x9E37_79B9_7F4A_7C15_u64;
    let mut failures = Vec::new();
    for (instruction, function) in FORMS {
        let behavior = X86_64::list_known_behaviors(instruction).next().expect("a behavior");
        let generated: Vec<ExecutionState<'static, X86_64>> = generator.generate_states(instruction).collect();
        // A state at the page start under FNINIT's FCW with every operand ONE, whose registers the random ones replace.
        let template = generated
            .iter()
            .find(|state| {
                let x87 = &state.processor.x87;
                x87.control == INITIAL_CONTROL
                    && x87.status & 0x3800 == 0
                    && x87.tags == if function.is_binary() { 0b11 } else { 0b1 }
                    && x87.read(0) == ONE
                    && (!function.is_binary() || x87.read(1) == ONE)
            })
            .expect("a state with every operand 1")
            .clone();
        let exponents = find_exponents(function);
        let mut states = generated;
        for control in [0x037F, 0x077F, 0x0B7F, 0x0F7F] {
            for index in 0..RANDOM_ARGUMENTS {
                let mut state = template.clone();
                state.processor.x87.control = control;
                let negative = index % 2 == 1 && function != Function::YLogX;
                let x87 = &mut state.processor.x87;
                let (first, second) = (x87.to_physical(0), x87.to_physical(1));
                x87.registers[first] = make_value(&mut random, exponents[0], negative);
                if function.is_binary() {
                    x87.registers[second] = make_value(&mut random, exponents[1], index % 3 == 0);
                }
                states.push(state);
            }
        }
        // The largest error rounding to nearest, and under the other rounding controls.
        let (mut checked, mut largest) = (0, [0f64; 2]);
        for state in states {
            let before = &state.processor.x87;
            let (top, next) = (before.read(0), before.read(1));
            let Ok(simulated) = behavior.simulate_state(state.clone());
            let undecided = simulated.processor.x87_registers_undecided;
            let Ok(after) = machine.execute_single_instruction(state.clone()) else {
                continue;
            };
            if undecided == 0 || after.processor.exception.is_some() {
                continue;
            }
            let x87 = &after.processor.x87;
            // An overflow delivers an infinity or the largest value, and an unmasked one or an unmasked underflow a
            // value whose exponent is moved by 24,576: none is the computed value rounded.
            if x87.status & !before.status & (OVERFLOW | UNDERFLOW) != 0 && (before.control & UNDERFLOW == 0 || x87.status & OVERFLOW != 0) {
                continue;
            }
            // Each result's register after the form, and the function that gives it.
            let results: Vec<(usize, Function)> = match function {
                Function::Tangent => vec![(1, Function::Tangent)],
                Function::SineCosine => vec![(1, Function::Sine), (0, Function::Cosine)],
                _ => vec![(0, function)],
            };
            let bound = if before.control & 0x0C00 == 0 { 1.0 } else { 1.5 };
            for (index, part) in results {
                let Some(exact) = find_unrounded(part, top, next) else {
                    continue;
                };
                let error = measure_error(x87.read(index), exact);
                checked += 1;
                let directed = usize::from(bound > 1.0);
                largest[directed] = largest[directed].max(error);
                if error >= bound {
                    failures.push(format!(
                        "{instruction:?} fcw {:#06x} st0 {top:#022x} st1 {next:#022x}: chip {:#022x}, {error:.3} units in the last place",
                        before.control,
                        x87.read(index)
                    ));
                }
            }
        }
        println!(
            "{instruction:?}: {checked} results; the largest error {:.3} units in the last place rounding to nearest, {:.3} otherwise",
            largest[0], largest[1]
        );
    }
    assert!(failures.is_empty(), "{} results beyond the bound:\n{}", failures.len(), failures.join("\n"));
}
