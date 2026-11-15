//! The browser runner: a flat blob run in Chrome by `browser/runner.js`, with the translator built for wasm32
//! from `browser/` so that code is translated at run time there too.

use crate::programs::{Blob, LOAD, Ran, placement};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

fn package() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("browser")
}

fn target() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/browser")
}

/// Build the translator for the browser, once per process, and return the module's path.
pub fn translator() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let status = Command::new(cargo)
                .args([
                    "build",
                    "--release",
                    "--quiet",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--target-dir",
                ])
                .arg(target())
                .current_dir(package())
                .status()
                .expect("run cargo for the browser translator");
            assert!(status.success(), "the browser translator did not build");
            target().join("wasm32-unknown-unknown/release/wcontainers_browser.wasm")
        })
        .clone()
}

/// What happened in Chrome: the program's output and status, and the runner's report.
pub struct Report {
    pub ran: Ran,
    /// The slow paths taken, as `(reason, count)`.
    pub counters: Vec<(String, u64)>,
    pub user_agent: String,
}

impl Report {
    pub fn count(&self, reason: &str) -> u64 {
        self.counters
            .iter()
            .find(|(name, _)| name == reason)
            .map_or(0, |c| c.1)
    }
}

/// Run a blob through the translator at look-ahead none in Chrome, starting at its first byte.
pub fn translated(blob: &Blob) -> Report {
    let place = placement(blob);
    let directory = target().join("cases").join(&blob.name);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::copy(translator(), directory.join("translator.wasm")).unwrap();
    std::fs::write(directory.join("blob.bin"), &blob.bytes).unwrap();
    std::fs::write(directory.join("stack.bin"), &place.stack).unwrap();
    let regions: Vec<Value> = place
        .regions
        .iter()
        .map(|(range, permissions)| serde_json::json!([range.start, range.end, permissions.bits()]))
        .collect();
    let setup = serde_json::json!({
        "codeBytes": blob.code_bytes,
        "load": LOAD,
        "memoryBytes": place.memory_bytes,
        "regions": regions,
        "sp": place.sp,
    });
    std::fs::write(directory.join("case.json"), setup.to_string()).unwrap();

    let output = Command::new("node")
        .arg(package().join("run.mjs"))
        .arg(&directory)
        .output()
        .expect("run node (from the Nix shell)");
    let errors = String::from_utf8_lossy(&output.stderr);
    eprint!("{errors}");
    assert!(output.status.success(), "the browser runner failed");
    let result: Value = serde_json::from_slice(&output.stdout).expect("the runner's JSON");
    if let Some(error) = result.get("error") {
        panic!(
            "the browser runner failed: {}",
            error.as_str().unwrap_or_default()
        );
    }
    let bytes = |name: &str| -> Vec<u8> {
        result[name]
            .as_array()
            .expect("bytes")
            .iter()
            .map(|b| b.as_u64().unwrap() as u8)
            .collect()
    };
    let stopped = &result["stopped"];
    let status = match stopped["kind"].as_str() {
        // Linux reports the low 8 bits of the status.
        Some("finished") => stopped["status"].as_i64().unwrap() as i32 & 0xff,
        _ => panic!(
            "the program stopped without exiting in Chrome: {stopped} at {}",
            result["pc"]
        ),
    };
    let counters = result["counters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["reason"].as_str().unwrap().to_string(),
                c["count"].as_u64().unwrap(),
            )
        })
        .collect();
    Report {
        ran: Ran {
            stdout: bytes("stdout"),
            stderr: bytes("stderr"),
            status,
        },
        counters,
        user_agent: result["userAgent"].as_str().unwrap_or_default().to_string(),
    }
}
