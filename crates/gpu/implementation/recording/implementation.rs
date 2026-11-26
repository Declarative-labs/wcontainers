//! The recording back end of the graphics interface ([`Recorder`]): it keeps every object's bytes on the
//! CPU, runs each call as it is recorded and checks the interface's rules.

mod compute;
mod draw;
mod program;
mod recorder;
#[cfg(test)]
mod sequence;
mod stream;
mod texel;
#[cfg(test)]
mod unit_tests;

#[cfg(test)]
use recorder::Id;
use recorder::{Frame, Obj, Pass, Touched};
use stream::Call;
use wcontainers_gpu::{caps::Caps, error::LossReason, helpers::Helpers, slow::SlowPaths};

/// The recording back end.
pub struct Recorder {
    pub(crate) caps: Caps,
    pub(crate) slow: SlowPaths,
    pub(crate) helpers: Helpers<Recorder>,
    pub(crate) lost: Option<LossReason>,
    pub(crate) objects: Vec<Option<Obj>>,
    pub(crate) pass: Option<Pass>,
    pub(crate) submits: u64,
    /// The time timestamps give, in nanoseconds: one for each call run, nested ones too.
    pub(crate) clock: u64,
    /// Every base call, in order, with its answer.
    pub(crate) calls: Vec<Call>,
    /// How deep the calls being made are inside another base call, whose own entry is the one kept.
    pub(crate) nested: u32,
    /// Bytes commands read or wrote, and `Ordered` writes wrote, until the caller waits for their
    /// submit: what no `NoOverwrite` write may touch.
    pub(crate) touched: Vec<Touched>,
    /// Every rule of the interface a caller broke, and every call this back end could not carry out.
    /// Either loses the device.
    pub(crate) violations: Vec<String>,
    /// Indexed draws dropped whole because a vertex they fetch lies past its stream's end, the
    /// harshest outcome the interface allows them (`DrawRange`'s doc).
    pub(crate) dropped_draws: u64,
    /// Every frame presented.
    pub(crate) shown: Vec<Frame>,
}

/// What a recording back end is made from.
#[derive(Debug)]
pub struct RecorderConfig {
    /// What it declares; none declares every feature it records and every format it holds.
    pub caps: Option<Caps>,
}

#[cfg(test)]
use stream::replay;
