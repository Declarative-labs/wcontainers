//! A demonstration of the library: runs one container to its end and exits with its status.
//!
//! wcontainers (--oci DIRECTORY | --tar FILE) [--config config.json] [--cache DIRECTORY]
//!             [--look-ahead full|none|N] [--size] [-- ARGUMENTS...]
//!
//! What runs is the OCI image's configuration converted, or the runtime's base configuration for a
//! plain tar, or `--config`'s; the arguments, when given, replace `process.args`.

use std::path::PathBuf;
use std::process::ExitCode;

use wcontainers_arm64::Arm64;
use wcontainers_runtime::{
    BackEnd, Ending, Image, LocalSystem, Machine, Runtime, RuntimeSettings, Warned, spec_from_json,
};
use wcontainers_wasmtime::{CpuSide, LookAhead, Optimise, Tuning};

type Back = CpuSide<Arm64>;

/// Docker's exit statuses for a program not found and for its own failures.
const NOT_FOUND: u8 = 127;
const RUNTIME_FAILED: u8 = 125;

/// A Linux wait status: the signal that ended the process in its low bits, else the exit status
/// above them; a shell reports a signal as this base plus its number.
const SIGNAL_BITS: i32 = 0x7f;
const EXIT_SHIFT: u32 = 8;
const SIGNAL_BASE: u8 = 128;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("wcontainers: {error}");
            match error.downcast_ref::<wcontainers_runtime::Error>() {
                Some(wcontainers_runtime::Error::NotFound(_)) => ExitCode::from(NOT_FOUND),
                _ => ExitCode::from(RUNTIME_FAILED),
            }
        }
    }
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
    let mut arguments = pico_args::Arguments::from_env();
    let oci: Option<PathBuf> = arguments.opt_value_from_str("--oci")?;
    let tar: Option<PathBuf> = arguments.opt_value_from_str("--tar")?;
    let config: Option<PathBuf> = arguments.opt_value_from_str("--config")?;
    let cache: Option<PathBuf> = arguments.opt_value_from_str("--cache")?;
    let look_ahead = match arguments
        .opt_value_from_str::<_, String>("--look-ahead")?
        .as_deref()
    {
        None | Some("full") => LookAhead::Full,
        Some("none") => LookAhead::None,
        Some(n) => LookAhead::Bounded(n.parse()?),
    };
    let optimise = if arguments.contains("--size") {
        Optimise::Size
    } else {
        Optimise::Speed
    };
    let rest: Vec<String> = arguments
        .finish()
        .into_iter()
        .map(|a| a.into_string().map_err(|a| format!("{a:?} is not text")))
        .collect::<Result<_, _>>()?;
    let command: Vec<String> = match rest.split_first() {
        Some((dashes, command)) if dashes == "--" => command.to_vec(),
        Some((unknown, _)) => return Err(format!("unexpected argument {unknown}").into()),
        None => Vec::new(),
    };

    let image = match (oci, tar) {
        (Some(directory), None) => Image::oci(&directory, Back::ARCHITECTURE)?,
        (None, Some(file)) => Image::tar(&file)?,
        _ => return Err("give one of --oci and --tar".into()),
    };
    let mut spec = match config {
        Some(path) => spec_from_json(
            &std::fs::read(&path)
                .map_err(|error| wcontainers_runtime::Error::Io { path, error })?,
        )?,
        None => image.spec()?,
    };
    if !command.is_empty() {
        let mut process = spec.process().clone().unwrap_or_default();
        process.set_args(Some(command));
        spec.set_process(Some(process));
    }

    let runtime = Runtime::<Back>::new(&RuntimeSettings { cache });
    let system = LocalSystem::new(Box::new(std::io::stdout()), Box::new(std::io::stderr()));
    let tuning = Tuning {
        look_ahead,
        optimise,
    };
    let mut container =
        runtime.container(&spec, &image, &tuning, &Machine { offer: () }, system)?;
    let Ok(Ending::Finished { status }) = container.run();
    if let Some(error) = container.start_failure() {
        eprintln!("wcontainers: the program could not be started: {error:?}");
    }
    for Warned {
        reason, count, cpu, ..
    } in container.system().warnings()
    {
        let side = if *cpu { "back end" } else { "kernel" };
        eprintln!("wcontainers: {side}: {reason}: {count}");
    }
    // As a shell reports it.
    Ok(if status & SIGNAL_BITS == 0 {
        (status >> EXIT_SHIFT) as u8
    } else {
        SIGNAL_BASE + (status & SIGNAL_BITS) as u8
    })
}
