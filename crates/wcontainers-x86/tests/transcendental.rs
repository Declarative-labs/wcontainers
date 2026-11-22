//! The x87 unit's transcendental instructions against correctly rounded values worked out with mpmath at 600
//! bits and more, with the reference's own rules of plan 5.8 (`transcendental.txt`: instruction, rounding
//! control, ST(0), ST(1), the result, its C1 and precision flags, and for `fsincos` the sine, in hex), the
//! trigonometric ones with the argument reduced by Intel's 66-bit π.

use iced_x86::Mnemonic;
use wcontainers_x86::Fxsave;
use wcontainers_x86::x87::{
    C1, CONTROL_AT_START, Code, Extended, FIELD, Operation, PRECISION, ROUNDING_SHIFT, Unit,
    above_exact,
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
        let fields = line.split(' ').collect::<Vec<_>>();
        let [name, mode, x, y, want, flags, ..] = fields[..] else {
            panic!("a case: {line}");
        };
        let (operation, two, pushed) = match name {
            "fsin" => (Operation::Sine, false, false),
            "fsincos" => (Operation::SineCosine, false, true),
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
        // `fptan` leaves the tangent in ST(1), `fsincos` the cosine in ST(0) and the sine in ST(1).
        let sine_cosine = operation == Operation::SineCosine;
        let got = unit.get(u8::from(pushed && !sine_cosine));
        let sine = fields.get(6).map(|s| (unit.get(1), hex(s)));
        let got_flags = unit.area.fsw & (C1 | PRECISION);
        let want_flags = u16::from_str_radix(flags, 16).expect("hex");
        count += 1;
        if got != hex(want)
            || got_flags != want_flags
            || sine.is_some_and(|(got, want)| got != want)
        {
            failures.push(format!(
                "{line}: got {:020x} flags {got_flags:03x}{}",
                got.to_bits(),
                sine.map_or(String::new(), |(got, _)| format!(
                    " sine {:020x}",
                    got.to_bits()
                ))
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {count} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Layer 1's test of the side of the exact value a result lies on agrees with every inexact correctly
/// rounded value's C1 outside the reference's own rules.
#[test]
fn the_exact_side_is_c1() {
    let mut failures = Vec::new();
    let (mut count, mut ruled) = (0, 0);
    for line in CASES.lines() {
        let fields = line.split(' ').collect::<Vec<_>>();
        let [name, _, x, y, want, flags, ..] = fields[..] else {
            panic!("a case: {line}");
        };
        let flags = u16::from_str_radix(flags, 16).expect("hex");
        if flags & PRECISION == 0 {
            continue;
        }
        let mnemonic = match name {
            "fsin" => Mnemonic::Fsin,
            "fsincos" => Mnemonic::Fsincos,
            "fcos" => Mnemonic::Fcos,
            "fptan" => Mnemonic::Fptan,
            "f2xm1" => Mnemonic::F2xm1,
            "fyl2x" => Mnemonic::Fyl2x,
            "fyl2xp1" => Mnemonic::Fyl2xp1,
            "fpatan" => Mnemonic::Fpatan,
            _ => panic!("an instruction: {name}"),
        };
        count += 1;
        match above_exact(mnemonic, hex(x), hex(y), hex(want)) {
            None => ruled += 1,
            Some(up) if up != (flags & C1 != 0) => failures.push(line),
            Some(_) => {}
        }
    }
    println!("{count} inexact values, {ruled} under the reference's own rules");
    assert!(
        failures.is_empty(),
        "{} of {count} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
