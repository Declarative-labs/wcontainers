//! A demonstration of the library: runs one container to its end and exits with its status.
//!
//! wcon (--oci DIRECTORY | --tar FILE) [--config config.json] [--cache DIRECTORY]
//!      [--look-ahead full|none|N] [--size] [--volume SOURCE:DESTINATION]
//!      [--publish ADDRESS:PORT:CONTAINER_PORT]... [--interactive] [--tty] [-- ARGUMENTS...]
//!
//! What runs is the OCI image's configuration converted, or the runtime's base configuration for a
//! plain tar, or `--config`'s; the arguments, when given, replace `process.args`. `--volume` adds a
//! bind mount of a host directory to it; `--publish` lets the host reach a TCP port of the container.
//! `--interactive` gives the container this program's standard input, and `--tty` sets
//! `process.terminal`, as `docker run` takes them.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use wcontainers_arm64::Arm64;
use wcontainers_runtime::{
    Ending, Executor, GuestMachine, Image, LocalSystem, Runtime, RuntimeConfig, Warned, bind_mount,
    spec_from_json,
};
use wcontainers_wasmtime::{CpuSide, LookAhead, Optimise, TranslatorConfig};

type Back = CpuSide<Arm64>;

/// The warning for a property of the configuration that is ignored, with its path and count.
const IGNORED: &str = "a property the specification does not define, or an annotation, ignored";

/// The program's options, as `docker run` names those it shares.
const OCI: &str = "--oci";
const TAR: &str = "--tar";
const CONFIG: &str = "--config";
const CACHE: &str = "--cache";
const LOOK_AHEAD: &str = "--look-ahead";
const VOLUME: &str = "--volume";
const PUBLISH: &str = "--publish";
const INTERACTIVE: &str = "--interactive";
const TTY: &str = "--tty";
const SIZE: &str = "--size";
/// What ends the options: the arguments after it are `process.args`.
const END_OF_OPTIONS: &str = "--";
/// `--look-ahead`'s words; any other value is a bounded look-ahead's length.
const LOOK_AHEAD_FULL: &str = "full";
const LOOK_AHEAD_NONE: &str = "none";

/// What separates the parts of `--volume` and `--publish`.
const SEPARATOR: char = ':';

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
            eprintln!("wcon: {error}");
            match error.downcast_ref::<wcontainers_runtime::Error>() {
                Some(wcontainers_runtime::Error::NotFound(_)) => ExitCode::from(NOT_FOUND),
                _ => ExitCode::from(RUNTIME_FAILED),
            }
        }
    }
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
    let mut arguments = pico_args::Arguments::from_env();
    let oci: Option<PathBuf> = arguments.opt_value_from_str(OCI)?;
    let tar: Option<PathBuf> = arguments.opt_value_from_str(TAR)?;
    let config: Option<PathBuf> = arguments.opt_value_from_str(CONFIG)?;
    let cache: Option<PathBuf> = arguments.opt_value_from_str(CACHE)?;
    let look_ahead = match arguments
        .opt_value_from_str::<_, String>(LOOK_AHEAD)?
        .as_deref()
    {
        None | Some(LOOK_AHEAD_FULL) => LookAhead::Full,
        Some(LOOK_AHEAD_NONE) => LookAhead::None,
        Some(n) => LookAhead::Bounded(n.parse()?),
    };
    let volume: Option<String> = arguments.opt_value_from_str(VOLUME)?;
    let published: Vec<String> = arguments.values_from_str(PUBLISH)?;
    let interactive = arguments.contains(INTERACTIVE);
    let tty = arguments.contains(TTY);
    let optimise = if arguments.contains(SIZE) {
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
        Some((dashes, command)) if dashes == END_OF_OPTIONS => command.to_vec(),
        Some((unknown, _)) => return Err(format!("unexpected argument {unknown}").into()),
        None => Vec::new(),
    };

    let image = match (oci, tar) {
        (Some(directory), None) => Image::oci(&directory, Back::ARCHITECTURE)?,
        (None, Some(file)) => Image::tar(&file)?,
        _ => return Err(format!("give one of {OCI} and {TAR}").into()),
    };
    let mut spec = match config {
        Some(path) => {
            let (spec, ignored) = spec_from_json(
                &std::fs::read(&path)
                    .map_err(|error| wcontainers_runtime::Error::Io { path, error })?,
            )?;
            for (count, path) in (1..).zip(&ignored) {
                eprintln!("wcon: configuration: {IGNORED}: {path}: {count}");
            }
            spec
        }
        None => image.spec()?,
    };
    if let Some(volume) = volume {
        let (source, destination) = volume
            .split_once(SEPARATOR)
            .ok_or(format!("{VOLUME} is SOURCE{SEPARATOR}DESTINATION"))?;
        let mut mounts = spec.mounts().clone().unwrap_or_default();
        mounts.push(bind_mount(source.as_ref(), destination));
        spec.set_mounts(Some(mounts));
    }
    if !command.is_empty() || tty {
        let mut process = spec.process().clone().unwrap_or_default();
        if !command.is_empty() {
            process.set_args(Some(command));
        }
        if tty {
            process.set_terminal(Some(true));
        }
        spec.set_process(Some(process));
    }

    let runtime = Runtime::<Back>::new(&RuntimeConfig { cache });
    let mut system = LocalSystem::new(Box::new(std::io::stdout()), Box::new(std::io::stderr()));
    if interactive {
        system.input(Box::new(std::io::stdin()));
    }
    for publish in published {
        let (at, port) = publish.rsplit_once(SEPARATOR).ok_or(format!(
            "{PUBLISH} is ADDRESS{SEPARATOR}PORT{SEPARATOR}CONTAINER_PORT"
        ))?;
        system.publish(port.parse()?, at.parse::<SocketAddr>()?);
    }
    let translator_config = TranslatorConfig {
        look_ahead,
        optimise,
    };
    let mut container = runtime.container(
        &spec,
        &image,
        &translator_config,
        &GuestMachine { offer: () },
        system,
    )?;
    let Ok(Ending::Finished { status }) = container.run();
    if let Some(error) = container.start_failure() {
        eprintln!("wcon: the program could not be started: {error:?}");
    }
    for Warned {
        reason, count, cpu, ..
    } in container.system().warnings()
    {
        if cpu {
            eprintln!("wcon: executor: {reason}: {count}");
        } else {
            eprintln!("wcon: kernel: {reason}: {count}");
        }
    }
    // As a shell reports it.
    Ok(if status & SIGNAL_BITS == 0 {
        (status >> EXIT_SHIFT) as u8
    } else {
        SIGNAL_BASE + (status & SIGNAL_BITS) as u8
    })
}
