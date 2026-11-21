//! The x87 unit's transcendental instructions against correctly rounded values worked out with mpmath at 600
//! bits (`transcendental.txt`: instruction, rounding control, ST(0), ST(1) and the result, as 80-bit hex), the
//! trigonometric ones with the argument reduced by Intel's 66-bit π.

use wcontainers_x86::Fxsave;
use wcontainers_x86::x87::{
    CONTROL_AT_START, Code, Extended, FIELD, Operation, ROUNDING_SHIFT, Unit,
};

const CASES: &str = include_str!("transcendental.txt");
/// The rounding control's modes in the order of its field.
const MODES: &str = "nduz";

fn hex(s: &str) -> Extended {
    Extended::from_bits(u128::from_str_radix(s, 16).expect("hex"))
}

#[test]
fn transcendental_results_are_correctly_rounded() {
    let mut failures = Vec::new();
    let mut count = 0;
    for line in CASES.lines() {
        let [name, mode, x, y, want] = line.split(' ').collect::<Vec<_>>()[..] else {
            panic!("a case: {line}");
        };
        let (operation, two, pushed) = match name {
            "fsin" => (Operation::Sine, false, false),
            "fcos" => (Operation::Cosine, false, false),
            "fptan" => (Operation::Tangent, false, true),
            "f2xm1" => (Operation::ExponentMinusOne, false, false),
            "fyl2x" => (Operation::Logarithm, true, false),
            "fyl2xp1" => (Operation::LogarithmPlusOne, true, false),
            "fpatan" => (Operation::ArcTangent, true, false),
            _ => panic!("an instruction: {name}"),
        };
        let rounding = MODES.find(mode).expect("a mode") as u16;
        let mut area = vec![0u8; size_of::<Fxsave>()];
        let mut code = Code::new(operation);
        code.pops = u8::from(two);
        let mut unit = Unit::new(&mut area, code, 0);
        unit.area.fcw = CONTROL_AT_START & !(FIELD << ROUNDING_SHIFT) | rounding << ROUNDING_SHIFT;
        // ST(0) in R7, so that a push lands in R6.
        unit.area.fsw =
            (wcontainers_x86::x87::REGISTERS as u16 - 1) << wcontainers_x86::x87::TOP_SHIFT;
        unit.set(1, hex(y));
        unit.set(0, hex(x));
        unit.run(0, 0);
        let got = unit.get(u8::from(pushed));
        count += 1;
        if got != hex(want) {
            failures.push(format!("{line}: got {:020x}", got.to_bits()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {count} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
