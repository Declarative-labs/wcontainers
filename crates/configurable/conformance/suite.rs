//! The tests every implementation of wcontainers-configurable runs, written against its traits alone.
//!
//! Each takes the implementor with a handle and a setting to give it, made by the suite of a trait that
//! requires `Configurable`, which checks the setting's effect itself.

use wcontainers_configurable::Configurable;

/// Checks that setting the same value twice gives the answer setting it once gave.
pub fn setting_twice_equals_setting_once<C, Handle, Setting>(
    (configurable, handle, setting): (C, Handle, Setting),
) where
    C: Configurable<Handle, Setting>,
    Setting: Clone,
{
    let once = configurable.configure(&handle, setting.clone());
    let twice = configurable.configure(&handle, setting);
    assert_eq!(twice, once);
}

/// Runs each suite function above as its own test on the value a requiring suite gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn setting_twice_equals_setting_once() {
            $crate::setting_twice_equals_setting_once($value);
        }
    };
}

#[cfg(test)]
mod suite {
    use core::cell::Cell;

    use wcontainers_abi::HostError;
    use wcontainers_configurable::Configurable;

    /// Takes a setting the first time and refuses it after, which the suite must catch.
    struct TakesOnce(Cell<bool>);

    impl Configurable<(), bool> for TakesOnce {
        fn configure(&self, _handle: &(), setting: bool) -> Result<(), HostError> {
            if self.0.replace(setting) && setting {
                Err(HostError::Errno(libc::EBUSY))
            } else {
                Ok(())
            }
        }
    }

    /// Holds the last setting, as a correct implementation does.
    struct Keeps(Cell<bool>);

    impl Configurable<(), bool> for Keeps {
        fn configure(&self, _handle: &(), setting: bool) -> Result<(), HostError> {
            self.0.set(setting);
            Ok(())
        }
    }

    crate::tests!((Keeps(Cell::new(false)), (), true));

    #[test]
    #[should_panic]
    fn catches_a_setting_taken_only_once() {
        crate::setting_twice_equals_setting_once((TakesOnce(Cell::new(false)), (), true));
    }
}
