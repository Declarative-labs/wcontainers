//! What the graphics fronts share on the way into a back end and no back end uses. It moved here out
//! of `wcontainers-gpu`, which holds only the interface; this is its home until the fronts' shared
//! code has one of its own.

mod combinations;
mod scissors;

pub use combinations::{Combination, CombinationTracker};
pub use scissors::cut_scissors;
