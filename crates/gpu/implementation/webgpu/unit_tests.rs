//! The back end's own tests of what no call of the interface reports.
//!
//! What its caches made (pipelines and bind groups cached, writes staged, textures renamed, rings made),
//! how its depth probe found 24-bit depth stored, and the checks shared with the scenes page, counted here
//! where the page cannot count them. Each needs a GPU, or lavapipe where the Vulkan loader names it alone;
//! `WEBGPU_API` picks wgpu's API (dx12, vulkan, metal).

use wcontainers_graphics_test::scene;

#[allow(dead_code)]
#[path = "tests/scenes/draw.rs"]
mod draw;

#[allow(dead_code)]
#[path = "tests/allowance/mod.rs"]
mod allowance;

#[allow(dead_code)]
#[path = "tests/scenes/renames.rs"]
mod renames;

#[path = "tests/scenes/pool.rs"]
mod pool;

#[allow(dead_code)]
#[path = "tests/scenes/views.rs"]
mod views;

#[allow(dead_code)]
#[path = "tests/scenes/clears.rs"]
mod clears;

#[allow(dead_code)]
#[path = "tests/scenes/depth16.rs"]
mod depth16;

#[allow(dead_code)]
#[path = "tests/scenes/gaps.rs"]
mod gaps;

#[allow(dead_code)]
#[path = "tests/scenes/prepare.rs"]
mod prepare;

#[allow(dead_code)]
#[path = "tests/scenes/pulled.rs"]
mod pulled;

#[allow(dead_code)]
#[path = "tests/scenes/queries.rs"]
mod queries;

mod bindings;
mod copies;
mod depth_probe;
mod first_draws;
mod pulled_costs;
mod query_sets;
mod scenes;
mod writes;

use crate::WebGpu;

/// Reads what the checks shared with the scenes page count from the back end's own counters.
fn made(b: &WebGpu) -> Option<draw::Made> {
    let c = b.counters();
    Some(draw::Made {
        pipelines: c.pipelines,
        bind_groups: c.bind_groups,
        texture_renames: c.texture_renames,
        spares_reused: c.spares_reused,
        spares_dropped: c.spares_dropped,
        query_rings: c.query_rings,
    })
}
