//! The tests every implementation of wcontainers-configurable runs, written against its traits alone.

use wcontainers_configurable::Configurable;

/// TODO: temporary placeholder: a suite given only a `Configurable` has no handle and no setting to call it with.
pub fn call_configurable_methods<C, Handle, Setting>(_configurable: C)
where
    C: Configurable<Handle, Setting>,
{
    todo!("this is a placeholder: the real checks of Configurable are not written yet");
}
