//! Checks of what an expectation compares, worked out by hand.

use crate::{ProcessorExpectation, SuppressibleLoad, ZERO_PROCESSOR};

/// A load of bytes into Z3 that may suppress from its first element on.
const BYTES: SuppressibleLoad = SuppressibleLoad {
    register: 3,
    element_bits: u8::BITS,
    first_suppressible: 0,
    first_fault_before: u16::MAX,
    when_read: 0x0F0E_0D0C_0B0A_0908_0706_0504_0302_0100,
    when_zeroed: 0,
    when_merged: 0,
    when_suppressed: 0,
    below_fault: 0,
};

#[test]
fn the_observed_first_fault_register_shows_the_first_suppressed_element() {
    assert_eq!(BYTES.find_first_suppressed(0x0007), 3);
    assert_eq!(BYTES.find_first_suppressed(u16::MAX), 16);
    assert_eq!(BYTES.find_first_suppressed(0), 0);
    // Elements whose bits were clear before may have been suppressed already.
    let low = SuppressibleLoad {
        first_fault_before: 0x00FF,
        ..BYTES
    };
    assert_eq!(low.find_first_suppressed(0x00FF), 8);
    assert_eq!(low.find_first_suppressed(0x000F), 4);
    // Never before the first element the load may suppress.
    let second = SuppressibleLoad {
        first_suppressible: 5,
        first_fault_before: 0,
        ..BYTES
    };
    assert_eq!(second.find_first_suppressed(0), 5);
    // A doubleword's eight bits clear together.
    let doublewords = SuppressibleLoad {
        element_bits: u64::BITS,
        ..BYTES
    };
    assert_eq!(doublewords.find_first_suppressed(0x00FF), 1);
    assert_eq!(doublewords.to_uncompared(0x00FF), (u128::MAX << 64, 0xFF00));
}

#[test]
fn a_suppressible_load_compares_its_lanes_below_the_first_suppressed() {
    let mut expected = ProcessorExpectation::new(ZERO_PROCESSOR);
    expected.processor.vector_registers[3] = 0x0F0E_0D0C_0B0A_0908_0706_0504_0302_0100;
    expected.processor.first_fault_register = u16::MAX;
    expected.suppressible_load = Some(BYTES);
    let mut observed = expected.processor.clone();
    assert!(expected.is_met_by(&observed));
    // Suppressed from element 3: each lane from it on holds what it read or zero, and FFR is clear from it on.
    observed.vector_registers[3] = 0x0F0E_0000_0000_0000_0000_0504_0002_0100;
    observed.first_fault_register = 0x0007;
    assert!(expected.is_met_by(&observed));
    // A lane from it on that holds neither.
    observed.vector_registers[3] ^= 1 << 120;
    assert!(!expected.is_met_by(&observed));
    observed.vector_registers[3] ^= 1 << 120;
    // A lane below the first suppressed differs.
    observed.vector_registers[3] ^= 1 << 8;
    assert!(!expected.is_met_by(&observed));
    observed.vector_registers[3] ^= 1 << 8;
    // FFR set again after a suppressed element.
    observed.first_fault_register = 0x0107;
    assert!(!expected.is_met_by(&observed));
    observed.first_fault_register = 0x0007;
    // Another register's lanes are all compared.
    observed.vector_registers[4] = 1;
    assert!(!expected.is_met_by(&observed));
    observed.vector_registers[4] = 0;
    // Without the suppressible load every lane is compared.
    expected.suppressible_load = None;
    assert!(!expected.is_met_by(&observed));
}

#[test]
fn uncompared_bits_of_a_register_and_nzcv_are_left_out() {
    let mut expected = ProcessorExpectation::new(ZERO_PROCESSOR);
    let mut observed = ZERO_PROCESSOR;
    observed.general_purpose_registers[5] = 0x1234;
    observed.condition_flags = crate::ConditionFlags::ZERO;
    assert!(!expected.is_met_by(&observed));
    expected.uncompared.general_purpose_registers[5] = 0xFFFF;
    assert!(!expected.is_met_by(&observed));
    expected.uncompared.condition_flags = crate::ConditionFlags::ZERO;
    assert!(expected.is_met_by(&observed));
    observed.general_purpose_registers[5] = 0x1_0000;
    assert!(!expected.is_met_by(&observed));
}
