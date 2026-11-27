//! The serving glue's first slice under its hostile C caller (caller.c): each case on a screen of the recording
//! implementation, alone in a process of its own; what C sees (the case's status) and what reached the implementation
//! (the recording's log, in order) are both checked. Built against the pinned Mesa with the `hostile` feature.
#![cfg(all(mesa, feature = "hostile"))]

use std::process::Command;
use std::sync::Arc;

use wcontainers_gallium_recording::Recording;

#[expect(unsafe_code, reason = "caller.c defines wc_caller_cases and wc_caller_case as declared here")]
unsafe extern "C" {
    fn wc_caller_cases() -> i32;
    fn wc_caller_case(address: usize, which: i32) -> i32;
}

/// The environment variable naming the one case a child process runs.
const CASE_VARIABLE: &str = "WCONTAINERS_GALLIUM_CASE";

/// What each case must leave in the log: the methods that reached the implementation, in order.
const EXPECTED: &[&[&str]] = &[
    &[
        "context_create",
        "create_blend_state",
        "bind_blend_state",
        "bind_blend_state",
        "delete_blend_state",
        "create_depth_stencil_alpha_state",
        "bind_depth_stencil_alpha_state",
        "delete_depth_stencil_alpha_state",
        "create_rasterizer_state",
        "bind_rasterizer_state",
        "delete_rasterizer_state",
        "create_sampler_state",
        "create_sampler_state",
        "bind_sampler_states",
        "delete_sampler_state",
        "delete_sampler_state",
        "create_vertex_elements_state",
        "bind_vertex_elements_state",
        "delete_vertex_elements_state",
        "destroy",
        "destroy",
    ],
    &["context_create", "destroy", "destroy"],
    &["context_create", "create_blend_state", "delete_blend_state", "destroy", "destroy"],
    &["context_create", "create_rasterizer_state", "delete_rasterizer_state", "destroy", "destroy"],
    &["context_create", "destroy", "destroy"],
    &["context_create", "destroy", "destroy"],
];

/// Arguments the first case's calls must have reached the implementation with (Mesa's values, flattened by C and
/// made owned by Rust): the sampler's level range (both of `float min_lod, max_lod;`), the blend, depth and
/// rasterizer fields, the vertex element's offset and the stage and slots of the bound samplers.
const ARGUMENTS: &[&str] = &[
    "min_lod: 1.5",
    "max_lod: 9.25",
    "rgb_src_factor: Blendfactor(1)",
    "independent_blend_enable: 1",
    "depth_func: CompareFunc(1)",
    "cull_face: 2",
    "src_offset: 16",
    "shader MesaShaderStage(4)",
];

/// One case on a new screen of the recording implementation: the case's status and the log.
fn run(which: i32) -> (i32, Vec<String>) {
    let recording = Recording::new();
    let log = Arc::clone(&recording.log);
    let address = wcontainers_gallium_direct::serve::serve(recording).expect("the screen is served");
    #[expect(unsafe_code, reason = "caller.c: wc_caller_case takes the address serve returned (serve.c's live struct pipe_screen) and a case number, and calls only that screen's pointers")]
    // SAFETY: the address is the screen serve made just now, not yet destroyed; the case destroys it last.
    let status = unsafe { wc_caller_case(address, which) };
    (status, log.calls())
}

/// Checks one case: its status 0, the methods that reached the implementation exactly those expected, and the first
/// case's arguments.
fn holds(which: i32) -> Result<(), String> {
    let (status, calls) = run(which);
    let names: Vec<&str> = calls.iter().map(|call| call.split(' ').next().unwrap_or_default()).collect();
    let expected = EXPECTED.get(usize::try_from(which).unwrap_or(usize::MAX)).ok_or("no such case")?;
    if status != 0 {
        return Err(format!("case {which}: C saw status {status}; calls {names:?}"));
    }
    if names != *expected {
        return Err(format!("case {which}: calls {names:?}, expected {expected:?}"));
    }
    if which == 0 {
        let all = calls.join("\n");
        let missing: Vec<&&str> = ARGUMENTS.iter().filter(|argument| !all.contains(**argument)).collect();
        if !missing.is_empty() {
            return Err(format!("case 0: arguments not received {missing:?}:\n{all}"));
        }
    }
    Ok(())
}

/// Every case, each alone in a process of its own (this test binary run again for one case).
#[test]
fn every_case_alone() {
    #[expect(unsafe_code, reason = "caller.c: wc_caller_cases returns a number, reading nothing")]
    // SAFETY: a plain call of our own C function.
    let cases = unsafe { wc_caller_cases() };
    assert_eq!(usize::try_from(cases).ok(), Some(EXPECTED.len()), "a log expected for every case of caller.c");
    let binary = std::env::current_exe().expect("the test binary");
    for which in 0..cases {
        let output = Command::new(&binary).args(["--exact", "one_case", "--nocapture", "--test-threads=1"]).env(CASE_VARIABLE, which.to_string()).output().expect("the case's process runs");
        let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
        println!("SERVED CASE {which}: {}", if output.status.success() && text.contains(&format!("CASE {which} HELD")) { "held".to_string() } else { format!("FAILED {:?} {text}", output.status) });
        assert!(output.status.success() && text.contains(&format!("CASE {which} HELD")), "case {which}: {text}");
    }
}

/// The case WCONTAINERS_GALLIUM_CASE names; where it names none, every case in this process.
#[test]
fn one_case() {
    let cases: Vec<i32> = match std::env::var(CASE_VARIABLE) {
        Ok(which) => vec![which.parse().expect("a case number")],
        Err(_) => (0..i32::try_from(EXPECTED.len()).unwrap_or(0)).collect(),
    };
    for which in cases {
        match holds(which) {
            Ok(()) => println!("CASE {which} HELD"),
            Err(problem) => panic!("{problem}"),
        }
    }
}
