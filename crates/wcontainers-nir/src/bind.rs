//! The binding rule of the common form (plan 6.4) as the NIR route applies it: each stage's resources
//! in its own set, each kind numbered from its range's start in the order of the program's own
//! (set, binding), recorded in the reflection with the program's set as `space` and its binding as
//! `register`.

use wcontainers_gpu::{ShaderStage, SlotKind};

/// The kinds the route numbers, in the order of the bridge's `enum wc_slot_kind`.
pub const KINDS: [SlotKind; 4] = [
    SlotKind::Uniform,
    SlotKind::Sampler,
    SlotKind::Texture,
    SlotKind::Storage,
];

/// The set a stage's own resources bind in: the vertex stage's (and a compute program's) first, the
/// fragment stage's next.
pub const fn stage_set(stage: ShaderStage) -> u32 {
    match stage {
        ShaderStage::Vertex | ShaderStage::Compute => 0,
        ShaderStage::Fragment => 1,
    }
}

/// Why the rule refused a program, by the bridge's `enum wc_bind_result`.
pub const BEYOND_LIMIT: &str = "more resources of a kind than its range holds";
pub const UNBOUNDED: &str = "an array of resources without a size needs a resource table";
pub const OUT_OF_MEMORY: &str = "out of memory";
