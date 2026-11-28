//! Which offers have address windows: only those whose addresses are the 2^48 of a 48-bit virtual address.

use wcontainers_arm64::{Addresses, Arm64, KeptRegisters, Offer};
use wcontainers_guest_cpu::Guest;

#[test]
fn only_an_offer_of_windows_has_address_windows() {
    assert!(!Arm64::address_windows(Offer::default()));
    for kept_registers in [KeptRegisters::AnsweredInPlace, KeptRegisters::Trapped] {
        let four_gib = Offer {
            kept_registers,
            addresses: Addresses::FourGib,
        };
        let windows = Offer {
            kept_registers,
            addresses: Addresses::Windows,
        };
        assert!(!Arm64::address_windows(four_gib), "{four_gib:?}");
        assert!(Arm64::address_windows(windows), "{windows:?}");
    }
}
