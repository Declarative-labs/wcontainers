//! The tests every implementation of wcontainers-privilege-level runs, written against its traits alone.

use wcontainers_privilege_level::PrivilegeLevel;

/// Lists each instruction specific to the level once.
pub fn lists_each_specific_instruction_once<T: PrivilegeLevel>(_: T) {
    let listed: Vec<T::PrivilegedInstruction> = T::list_specific_instructions().collect();
    for (index, instruction) in listed.iter().enumerate() {
        assert!(
            !listed[..index].contains(instruction),
            "specific instruction {index} is listed twice"
        );
    }
}

/// Runs each suite function above as its own test on a value made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn lists_each_specific_instruction_once() {
            $crate::lists_each_specific_instruction_once($value);
        }
    };
}
