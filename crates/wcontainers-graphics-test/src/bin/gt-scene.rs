//! Prints a scene's data as JSON for the renderers outside Rust, the browser page:
//! `gt-scene triangle` prints its size, clear colour and vertices (x, y, r, g, b each).

use wcontainers_graphics_test::scene::{self, Scene};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [name] = args.as_slice() else {
        anyhow::bail!("usage: gt-scene triangle");
    };
    anyhow::ensure!(
        Scene::from_name(name) == Some(Scene::Triangle),
        "only the triangle is drawn outside Rust"
    );
    let data = serde_json::json!({
        "width": scene::WIDTH,
        "height": scene::HEIGHT,
        "clear": scene::CLEAR,
        "vertices": scene::TRIANGLE,
    });
    println!("{data}");
    Ok(())
}
