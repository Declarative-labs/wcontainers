//! Testing for the graphics interface: the recording back end ([`Recorder`]), which keeps every
//! object's bytes on the CPU, runs each call as it is recorded and checks the interface's rules; and
//! the shared suite ([`mod@suite`], run with [`suite!`]) every back end runs through the interface.

mod draw;
pub mod program;
mod recorder;
pub mod stream;
pub mod suite;
pub mod texel;

pub use recorder::{Frame, Id, Recorder};
pub use stream::{Call, replay};
pub use suite::Host;
