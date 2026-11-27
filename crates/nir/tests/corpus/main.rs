//! The NIR route's corpus run (plan 15.3): every SPIR-V shader under `WC_NIR_CORPUS` through route
//! B, each shader in a child process of its own (Mesa aborts on some inputs), one JSON
//! line a shader into `WC_NIR_RESULTS`. Each converted shader is held to lavapipe running the
//! program as given (`compare`). Knobs: `WC_NIR_SHADERS` (how many, a sample drawn with
//! `WC_NIR_SEED`, which also seeds every shader's inputs; all when unset), `WC_NIR_INPUT_SETS` (the
//! vertices or fragments each run, each with inputs of its own), `WC_NIR_JOBS` (children at once). Without `WC_NIR_CORPUS`, or
//! without Mesa, it runs nothing. With `--godot-rd <Godot's tree> <folder>` it writes the sources of
//! Godot's RenderingDevice shader variants instead (`godot_rd`).

#[cfg(mesa)]
mod bgfx;
#[cfg(mesa)]
mod compare;
#[cfg(mesa)]
mod deqp;
#[cfg(mesa)]
mod entry;
#[cfg(mesa)]
mod glsl;
#[cfg(mesa)]
mod godot;
mod godot_rd;
#[cfg(mesa)]
mod interface;
#[cfg(mesa)]
mod lavapipe;
#[cfg(mesa)]
mod llvmpipe;
#[cfg(mesa)]
mod needs;
#[cfg(mesa)]
mod run;
mod table;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(at) = args.iter().position(|a| a == godot_rd::WRITE) {
        let [godot, out] = &args[at + 1..] else {
            panic!(
                "{} takes Godot's tree and an output folder",
                godot_rd::WRITE
            );
        };
        match godot_rd::write(godot.as_ref(), out.as_ref()) {
            Ok(written) => eprintln!("godot-rd: {written} stage sources in {out}"),
            Err(e) => panic!("godot-rd: {e}"),
        }
        return;
    }
    corpus();
}

#[cfg(not(mesa))]
fn corpus() {
    eprintln!("corpus: built without Mesa; nothing run");
}

#[cfg(mesa)]
fn corpus() {
    run::main();
}
