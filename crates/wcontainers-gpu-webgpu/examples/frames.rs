//! Where a real program's CPU time goes, from profiles (samply's conversion of Windows' sampled
//! traces, in the Firefox profiler's format) of one program run twice, to two last frames: per frame,
//! the difference of the two runs divided by the difference of their frames, so start-up and
//! shut-down cancel. Each sample goes to the innermost frame of a known part (the program, the
//! Direct3D front, the WebGPU or WebGL 2 back end, wgpu, ANGLE, naga, the shader converter, the
//! driver, the system), library code (`core`, `alloc`, `std`, `hashbrown`) to its caller's part;
//! and the samples under a back end frame are split the same way.
//!
//! Our libraries (those loaded from under `\\wc\\`) are named from their PDBs, found by library
//! name in the given directories; a sample in code inlined into a function counts as that
//! function's. Other libraries count by their name.
//!
//! With `--log`, the back end's counters a frame too: from the program's log of the long run,
//! the difference of its last report ([`wcontainers_gpu_webgpu::FRAME_REPORT`]) and the one at
//! half its frames.
//!
//!   frames [--log <output.log>] <process> <short.json> <short frames> <long.json> <long frames>
//!     <pdb dir>...

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use pdb::FallibleIterator;

use anyhow::{Context, Result, anyhow, ensure};
use serde_json::Value;

/// The parts a sample can belong to, in printing order.
const PARTS: [&str; 12] = [
    "program",
    "rig forwarder",
    "front",
    "interface",
    "back end",
    "wgpu",
    "angle",
    "naga",
    "shaders",
    "driver",
    "system",
    "unknown",
];

/// The part of a function, from the crate its name starts with; None for library code.
fn part_of_function(name: &str) -> Option<&'static str> {
    let name = name.trim_start_matches(['<', '&', '*', ' ']);
    let name = name.strip_prefix("mut ").unwrap_or(name);
    let name = name.strip_prefix("dyn ").unwrap_or(name);
    let krate = name.split("::").next()?;
    Some(match krate {
        "wcontainers_gpu_webgpu" | "wcontainers_gpu_webgl" | "glow" | "khronos_egl" => "back end",
        "wcontainers_gpu" => "interface",
        "wgpu" | "wgpu_core" | "wgpu_hal" | "wgpu_types" | "gpu_allocator" | "gpu_descriptor"
        | "range_alloc" | "ash" | "windows" | "windows_core" | "parking_lot"
        | "parking_lot_core" => "wgpu",
        "naga" => "naga",
        "wcontainers_shaders" | "dxbc_spirv" | "spirv" | "rspirv" => "shaders",
        "wcontainers_direct3d_11"
        | "wcontainers_direct3d_12"
        | "wcontainers_directx"
        | "wcontainers_directx_graphics_infrastructure"
        | "wcontainers_d3d12"
        | "d3d12"
        | "dxgi"
        | "d3d11" => "front",
        "wcontainers_graphics_test_forward" | "wcontainers_graphics_test" => "rig forwarder",
        _ => return None,
    })
}

/// The part of a frame in a library without a known function: by the library's name.
fn part_of_library(lib: &str, ours: bool) -> &'static str {
    let l = lib.to_ascii_lowercase();
    if l.starts_with("wcontainers_graphics_test_forward") {
        "rig forwarder"
    } else if l.starts_with("libglesv2") || l.starts_with("libegl") {
        "angle"
    } else if ours {
        "unknown"
    } else if l.starts_with("nv")
        || l.starts_with("d3d12core")
        || l.starts_with("d3d12")
        || l.starts_with("d3d11")
        || l.starts_with("dxgi")
        || l.starts_with("d3d10warp")
        || l.starts_with("vulkan")
        || l.starts_with("dxcore")
        || l.starts_with("d3dcompiler")
        || l.starts_with("dxcompiler")
        || l.starts_with("dxil")
        || l.starts_with("igd")
        || l.starts_with("amd")
    {
        "driver"
    } else if l.ends_with(".sys") || l.starts_with("ntoskrnl") {
        "system"
    } else if l.ends_with(".exe") || l.starts_with("sdl") || l.starts_with("godot") {
        "program"
    } else {
        "system"
    }
}

/// One library's procedures from its PDB: (start, length, name), sorted by start.
struct Procedures(Vec<(u32, u32, String)>);

impl Procedures {
    /// Reads the procedures and public symbols of the PDB at `path`.
    fn read(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut pdb = pdb::PDB::open(file)?;
        let map = pdb.address_map()?;
        let mut list = Vec::new();
        let dbi = pdb.debug_information()?;
        let mut modules = dbi.modules()?;
        while let Some(module) = modules.next()? {
            let Some(info) = pdb.module_info(&module)? else {
                continue;
            };
            let mut symbols = info.symbols()?;
            while let Some(symbol) = symbols.next()? {
                if let Ok(pdb::SymbolData::Procedure(p)) = symbol.parse()
                    && let Some(rva) = p.offset.to_rva(&map)
                {
                    list.push((rva.0, p.len, p.name.to_string().into_owned()));
                }
            }
        }
        let globals = pdb.global_symbols()?;
        let mut symbols = globals.iter();
        while let Some(symbol) = symbols.next()? {
            if let Ok(pdb::SymbolData::Public(p)) = symbol.parse()
                && let Some(rva) = p.offset.to_rva(&map)
            {
                list.push((rva.0, 0, p.name.to_string().into_owned()));
            }
        }
        list.sort_by_key(|e| e.0);
        Ok(Procedures(list))
    }

    /// The name of the procedure holding `rva`, or of the public symbol at or before it.
    fn name(&self, rva: u32) -> Option<&str> {
        let i = self.0.partition_point(|e| e.0 <= rva).checked_sub(1)?;
        let mut j = i;
        loop {
            let (start, len, name) = &self.0[j];
            if *len == 0 || rva < start + len {
                return Some(name);
            }
            j = j.checked_sub(1)?;
            if rva - self.0[j].0 > 1 << 24 {
                return None;
            }
        }
    }
}

fn read_json(path: &str) -> Result<Value> {
    let file = File::open(path).with_context(|| format!("opening {path}"))?;
    Ok(serde_json::from_reader(BufReader::new(file))?)
}

/// Our libraries' procedures, by library name: `<name>.pdb` from the first of `dirs` holding it.
fn our_procedures(dirs: &[PathBuf], libs: &[Value]) -> Result<HashMap<String, Procedures>> {
    let mut found = HashMap::new();
    for lib in libs {
        let (name, path) = (
            lib["name"].as_str().unwrap_or(""),
            lib["path"].as_str().unwrap_or("").to_ascii_lowercase(),
        );
        if !path.contains("\\wc\\") || found.contains_key(name) {
            continue;
        }
        let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
        if let Some(pdb) = dirs
            .iter()
            .map(|d| d.join(format!("{stem}.pdb")))
            .find(|p| p.exists())
        {
            found.insert(name.to_owned(), Procedures::read(&pdb)?);
        }
    }
    Ok(found)
}

/// Sample counts by part: own (each sample's innermost part), and under the back end.
#[derive(Default, Debug)]
struct Counts {
    own: HashMap<&'static str, f64>,
    under_back_end: HashMap<&'static str, f64>,
    samples: f64,
    /// Milliseconds a sample stands for.
    interval: f64,
    threads: HashMap<String, f64>,
    /// By what the innermost part's frame is: a function of ours, or another library.
    places: HashMap<String, f64>,
    /// The first and last sample's times of the process, in milliseconds.
    span: (f64, f64),
}

/// The times of a thread's samples, in milliseconds.
fn sample_times(thread: &Value) -> Vec<f64> {
    let samples = &thread["samples"];
    let mut t = 0.0;
    match samples["time"].as_array() {
        Some(times) => times.iter().filter_map(Value::as_f64).collect(),
        None => column(samples, "timeDeltas")
            .iter()
            .map(|d| {
                t += d.as_f64().unwrap_or(0.0);
                t
            })
            .collect(),
    }
}

fn strings_of(profile: &Value, thread: &Value) -> Vec<String> {
    let list = thread["stringArray"]
        .as_array()
        .or_else(|| thread["stringTable"].as_array())
        .or_else(|| profile["shared"]["stringArray"].as_array());
    list.map(|l| {
        l.iter()
            .map(|s| s.as_str().unwrap_or("").to_owned())
            .collect()
    })
    .unwrap_or_default()
}

fn column<'a>(table: &'a Value, name: &str) -> &'a [Value] {
    table[name].as_array().map_or(&[], Vec::as_slice)
}

/// Counts the samples of every thread of processes whose name contains `process`.
fn count(path: &str, process: &str, dirs: &[PathBuf]) -> Result<Counts> {
    let profile = read_json(path)?;
    let libs = profile["libs"].as_array().context("libs")?;
    let lib_names: Vec<String> = libs
        .iter()
        .map(|l| l["name"].as_str().unwrap_or("").to_owned())
        .collect();
    let procedures = our_procedures(dirs, libs)?;
    let lib_symbols: Vec<Option<&Procedures>> =
        lib_names.iter().map(|n| procedures.get(n)).collect();
    let mut counts = Counts {
        interval: profile["meta"]["interval"].as_f64().unwrap_or(1.0),
        ..Default::default()
    };
    let process = process.to_ascii_lowercase();
    for thread in profile["threads"].as_array().context("threads")? {
        let pname = thread["processName"]
            .as_str()
            .unwrap_or("")
            .to_ascii_lowercase();
        if !pname.contains(&process) {
            continue;
        }
        let strings = strings_of(&profile, thread);
        let (frames, funcs, resources) = (
            &thread["frameTable"],
            &thread["funcTable"],
            &thread["resourceTable"],
        );
        let (stack_prefix, stack_frame) = (
            column(&thread["stackTable"], "prefix"),
            column(&thread["stackTable"], "frame"),
        );
        let (frame_address, frame_func) = (column(frames, "address"), column(frames, "func"));
        let (func_name, func_resource) = (column(funcs, "name"), column(funcs, "resource"));
        let resource_lib = column(resources, "lib");
        // Every frame's part, None for library code, and what it is (its function or library).
        let mut place = Vec::with_capacity(frame_func.len());
        let part: Vec<Option<&'static str>> = (0..frame_func.len())
            .map(|f| {
                let func = frame_func[f].as_u64().unwrap_or(0) as usize;
                let resource = func_resource
                    .get(func)
                    .and_then(Value::as_i64)
                    .unwrap_or(-1);
                let lib = (resource >= 0)
                    .then(|| resource_lib.get(resource as usize).and_then(Value::as_u64))
                    .flatten()
                    .map(|l| l as usize);
                let address = frame_address.get(f).and_then(Value::as_i64).unwrap_or(-1);
                let names = lib
                    .and_then(|l| lib_symbols[l])
                    .and_then(|s| s.name(u32::try_from(address).ok()?));
                let fallback = func_name
                    .get(func)
                    .and_then(Value::as_u64)
                    .and_then(|i| strings.get(i as usize));
                let found = names
                    .into_iter()
                    .chain(fallback.map(String::as_str))
                    .find_map(|n| part_of_function(n).map(|p| (p, n)));
                let lib_name = lib.map_or("", |l| lib_names[l].as_str());
                place.push(match found {
                    Some((_, n)) => n.to_owned(),
                    None => lib_name.to_owned(),
                });
                let found = found.map(|(p, _)| p);
                let ours = lib_name.to_ascii_lowercase().starts_with("wcontainers");
                found.or_else(|| {
                    let by_lib = part_of_library(lib_name, ours);
                    // Library code inside our module goes to its caller.
                    (!(ours && by_lib == "unknown")).then_some(by_lib)
                })
            })
            .collect();
        let samples = &thread["samples"];
        let weights = column(samples, "weight");
        let times = sample_times(thread);
        if let (Some(first), Some(last)) = (times.first(), times.last()) {
            counts.span = match counts.span {
                (0.0, 0.0) => (*first, *last),
                (a, b) => (a.min(*first), b.max(*last)),
            };
        }
        let mut total = 0.0;
        for (i, stack) in column(samples, "stack").iter().enumerate() {
            let w = weights.get(i).and_then(Value::as_f64).unwrap_or(1.0);
            let Some(mut s) = stack.as_u64().map(|s| s as usize) else {
                continue;
            };
            let (mut own, mut under, mut at, mut caller) = (None, false, None, None);
            // The outermost frame outside the thread start-up libraries: what the thread runs.
            let mut root = None;
            loop {
                let f = stack_frame[s].as_u64().unwrap_or(0) as usize;
                let p = part[f];
                if own.is_none() && p.is_some() {
                    (own, at) = (p, Some(f));
                } else if own.is_some()
                    && caller.is_none()
                    && matches!(
                        p,
                        Some("front" | "interface" | "back end" | "wgpu" | "naga" | "shaders")
                    )
                {
                    caller = Some(f);
                }
                under |= p == Some("back end");
                let l = place[f].to_ascii_lowercase();
                if !matches!(
                    l.as_str(),
                    "ntdll.dll" | "kernel32.dll" | "kernelbase.dll" | "ntoskrnl.exe"
                ) {
                    root = Some(f);
                }
                match stack_prefix[s].as_u64() {
                    Some(prefix) => s = prefix as usize,
                    None => break,
                }
            }
            let own = own.unwrap_or("unknown");
            *counts.own.entry(own).or_default() += w;
            let label = at.map_or("?", |f| place[f].as_str());
            let key = match (own, caller) {
                ("system" | "driver", Some(c)) => format!("{own}: {label} <- {}", place[c]),
                ("system" | "driver", None) => format!(
                    "{own}: {label} in a thread of {}",
                    root.map_or("?", |f| place[f].as_str())
                ),
                _ => format!("{own}: {label}"),
            };
            *counts.places.entry(key).or_default() += w;
            if under {
                *counts.under_back_end.entry(own).or_default() += w;
            }
            total += w;
        }
        counts.samples += total;
        let name = thread["name"].as_str().unwrap_or("?").to_owned();
        *counts.threads.entry(name).or_default() += total;
    }
    ensure!(
        counts.samples > 0.0,
        "no samples of a process named like {process} in {path}"
    );
    Ok(counts)
}

/// The back end's reports in a program's log: frames presented, and each count by name.
fn reports(log: &Path) -> Result<Vec<(u64, HashMap<String, u64>)>> {
    let bytes = std::fs::read(log).with_context(|| format!("reading {}", log.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line
            .trim()
            .strip_prefix(wcontainers_gpu_webgpu::FRAME_REPORT)
        else {
            continue;
        };
        let mut tokens = rest.split_whitespace();
        let frames: u64 = tokens.next().context("a report without frames")?.parse()?;
        let counts = tokens
            .filter_map(|t| t.split_once('='))
            .map(|(k, v)| Ok((k.to_string(), v.parse()?)))
            .collect::<Result<_>>()?;
        out.push((frames, counts));
    }
    Ok(out)
}

/// Prints each count that changed a frame between the log's last report and the one before it
/// at half its frames (the same device's, as each device reports at powers of two).
fn print_counters(log: &Path) -> Result<()> {
    let reports = reports(log)?;
    let Some((last, at_last)) = reports.last() else {
        println!("  the log has no report of the back end's counters");
        return Ok(());
    };
    let Some((first, at_first)) = reports.iter().rev().find(|(f, _)| f * 2 == *last) else {
        println!("  the log has no report at half of {last} frames");
        return Ok(());
    };
    let frames = (last - first) as f64;
    let mut names: Vec<&String> = at_last.keys().collect();
    names.sort();
    print!("  back end counts a frame (frames {first} to {last} of the long run):");
    for name in names {
        let d = at_last[name] - at_first.get(name).copied().unwrap_or(0);
        if d > 0 {
            print!(" {name} {:.2};", d as f64 / frames);
        }
    }
    println!();
    Ok(())
}

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let log = match args.first().map(String::as_str) {
        Some("--log") if args.len() > 1 => {
            let log = PathBuf::from(args.remove(1));
            args.remove(0);
            Some(log)
        }
        _ => None,
    };
    let [process, short, short_frames, long, long_frames, dirs @ ..] = &args[..] else {
        return Err(anyhow!(
            "usage: frames [--log <output.log>] <process> <short.json> <short frames> <long.json> \
             <long frames> <pdb dir>..."
        ));
    };
    let dirs: Vec<PathBuf> = dirs.iter().map(PathBuf::from).collect();
    let (a, b) = (count(short, process, &dirs)?, count(long, process, &dirs)?);
    let frames: f64 = long_frames.parse::<f64>()? - short_frames.parse::<f64>()?;
    ensure!(frames > 0.0, "the long run must have more frames");
    let ms =
        |c: &Counts, m: &HashMap<&str, f64>, p: &str| m.get(p).copied().unwrap_or(0.0) * c.interval;
    println!(
        "{process}: {:.0} and {:.0} samples ({} ms each); per frame over {frames} frames:",
        a.samples, b.samples, b.interval
    );
    let per = |m: fn(&Counts) -> &HashMap<&'static str, f64>, p: &str| {
        (ms(&b, m(&b), p) - ms(&a, m(&a), p)) / frames
    };
    let total: f64 = PARTS.iter().map(|p| per(|c| &c.own, p)).sum();
    let wall = ((b.span.1 - b.span.0) - (a.span.1 - a.span.0)) / frames;
    println!("  wall time {wall:.3} ms a frame (first to last sample of the process)");
    println!("  CPU {total:.3} ms a frame, by innermost part:");
    for p in PARTS {
        let v = per(|c| &c.own, p);
        if v.abs() >= 0.0005 {
            println!("    {p}: {v:.3} ms ({:.1} %)", 100.0 * v / total);
        }
    }
    let under: f64 = PARTS.iter().map(|p| per(|c| &c.under_back_end, p)).sum();
    println!("  under the back end's own functions {under:.3} ms a frame:");
    for p in PARTS {
        let v = per(|c| &c.under_back_end, p);
        if v.abs() >= 0.0005 {
            println!("    {p}: {v:.3} ms ({:.1} %)", 100.0 * v / under.max(1e-9));
        }
    }
    // Where the per-frame time went, the largest first.
    let mut places: Vec<(f64, &String)> = b
        .places
        .iter()
        .map(|(k, v)| {
            (
                (v - a.places.get(k).copied().unwrap_or(0.0)) * b.interval / frames,
                k,
            )
        })
        .collect();
    places.sort_by(|x, y| y.0.total_cmp(&x.0));
    println!("  largest places a frame (innermost function of ours, or library):");
    for (ms, k) in places.iter().take(16) {
        println!("    {ms:.3} ms {k}");
    }
    let mut threads: Vec<_> = b.threads.iter().collect();
    threads.sort_by(|x, y| y.1.total_cmp(x.1));
    print!("  busiest threads of the long run (ms):");
    for (name, n) in threads.iter().take(4) {
        print!(" {name} {:.0};", *n * b.interval);
    }
    println!();
    if let Some(log) = log {
        print_counters(&log)?;
    }
    Ok(())
}
